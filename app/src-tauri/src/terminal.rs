use crate::fs::VaultPathState;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtyPair, PtySize, native_pty_system};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tauri::ipc::Channel;

const MAX_MESSAGE: usize = 4096;

pub(crate) struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

pub struct TerminalState(pub(crate) Arc<Mutex<HashMap<u32, Session>>>);

impl TerminalState {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }
}

impl Default for TerminalState {
    fn default() -> Self {
        Self::new()
    }
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.max(1),
        cols: cols.max(1),
        pixel_width: 0,
        pixel_height: 0,
    }
}

// On unix this runs $SHELL as a login shell (argv0 prefixed with `-`), which is what gives a GUI-launched app the PATH path_helper builds.
fn default_shell() -> CommandBuilder {
    CommandBuilder::new_default_prog()
}

fn complete_prefix_len(bytes: &[u8]) -> usize {
    match std::str::from_utf8(bytes) {
        Ok(_) => bytes.len(),
        Err(err) => match err.error_len() {
            None => err.valid_up_to(),
            Some(_) => bytes.len(),
        },
    }
}

fn take_utf8(pending: &mut Vec<u8>, bytes: &[u8]) -> String {
    pending.extend_from_slice(bytes);
    let cut = complete_prefix_len(pending).min(MAX_MESSAGE);
    let text = String::from_utf8_lossy(&pending[..cut]).into_owned();
    pending.drain(..cut);
    text
}

fn pump<R: Read>(mut reader: R, mut emit: impl FnMut(&str) -> bool) {
    let mut buf = [0u8; MAX_MESSAGE];
    let mut pending = Vec::with_capacity(MAX_MESSAGE + 4);

    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let text = take_utf8(&mut pending, &buf[..n]);
                if !text.is_empty() && !emit(&text) {
                    return;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }

    if !pending.is_empty() {
        emit(&String::from_utf8_lossy(&pending));
    }
}

fn spawn_session(
    id: u32,
    cwd: &str,
    cols: u16,
    rows: u16,
    on_output: Channel<String>,
    on_exit: Channel<i32>,
    sessions: &Arc<Mutex<HashMap<u32, Session>>>,
) -> Result<(), String> {
    let PtyPair { slave, master } = native_pty_system()
        .openpty(size(cols, rows))
        .map_err(|e| format!("Failed to open a PTY: {e}"))?;

    let mut cmd = default_shell();
    cmd.cwd(cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    if std::env::var_os("LANG").is_none() {
        cmd.env("LANG", "en_US.UTF-8");
    }

    let mut child = slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to start the shell: {e}"))?;
    // Holding the slave open would keep the master from ever seeing EOF, leaving the reader thread alive after the shell exits.
    drop(slave);

    let reader = master
        .try_clone_reader()
        .map_err(|e| format!("Failed to read from the PTY: {e}"))?;
    let writer = master
        .take_writer()
        .map_err(|e| format!("Failed to write to the PTY: {e}"))?;
    let killer = child.clone_killer();

    let replaced = sessions
        .lock()
        .map_err(|_| "Terminal state is poisoned".to_string())?
        .insert(
            id,
            Session {
                master,
                writer,
                killer,
            },
        );
    drop(replaced);

    std::thread::spawn(move || pump(reader, |text| on_output.send(text.to_owned()).is_ok()));

    let owned = Arc::clone(sessions);
    std::thread::spawn(move || {
        let code = child.wait().map_or(-1, |status| status.exit_code() as i32);
        let _ = on_exit.send(code);
        if let Ok(mut map) = owned.lock() {
            map.remove(&id);
        }
    });

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn pty_spawn(
    id: u32,
    cols: u16,
    rows: u16,
    on_output: Channel<String>,
    on_exit: Channel<i32>,
    vault_path_state: tauri::State<'_, VaultPathState>,
    state: tauri::State<'_, TerminalState>,
) -> Result<(), String> {
    let cwd = vault_path_state
        .0
        .lock()
        .map_err(|_| "Vault path is poisoned".to_string())?
        .clone();
    if cwd.trim().is_empty() {
        return Err("Open a vault before starting a terminal".into());
    }
    spawn_session(id, &cwd, cols, rows, on_output, on_exit, &state.0)
}

#[tauri::command]
#[specta::specta]
pub fn pty_write(
    id: u32,
    data: String,
    state: tauri::State<'_, TerminalState>,
) -> Result<(), String> {
    let mut sessions = state
        .0
        .lock()
        .map_err(|_| "Terminal state is poisoned".to_string())?;
    let session = sessions
        .get_mut(&id)
        .ok_or_else(|| format!("Terminal {id} is not running"))?;
    session
        .writer
        .write_all(data.as_bytes())
        .and_then(|_| session.writer.flush())
        .map_err(|e| format!("Failed to write to the terminal: {e}"))
}

#[tauri::command]
#[specta::specta]
pub fn pty_resize(
    id: u32,
    cols: u16,
    rows: u16,
    state: tauri::State<'_, TerminalState>,
) -> Result<(), String> {
    let sessions = state
        .0
        .lock()
        .map_err(|_| "Terminal state is poisoned".to_string())?;
    let session = sessions
        .get(&id)
        .ok_or_else(|| format!("Terminal {id} is not running"))?;
    session
        .master
        .resize(size(cols, rows))
        .map_err(|e| format!("Failed to resize the terminal: {e}"))
}

#[tauri::command]
#[specta::specta]
pub fn pty_kill(id: u32, state: tauri::State<'_, TerminalState>) -> Result<(), String> {
    let session = state
        .0
        .lock()
        .map_err(|_| "Terminal state is poisoned".to_string())?
        .remove(&id);
    if let Some(mut session) = session {
        let _ = session.killer.kill();
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn pty_kill_all(state: tauri::State<'_, TerminalState>) -> Result<(), String> {
    kill_all(&state);
    Ok(())
}

pub fn kill_all(state: &TerminalState) {
    let sessions: Vec<Session> = match state.0.lock() {
        Ok(mut map) => map.drain().map(|(_, session)| session).collect(),
        Err(_) => return,
    };
    for mut session in sessions {
        let _ = session.killer.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take_in_parts(chunks: &[&[u8]]) -> String {
        let mut pending = Vec::new();
        let mut out = String::new();
        for chunk in chunks {
            out.push_str(&take_utf8(&mut pending, chunk));
        }
        assert!(
            pending.is_empty(),
            "every byte should have been delivered: {pending:?}"
        );
        out
    }

    #[test]
    fn a_character_split_across_reads_arrives_whole() {
        let euro = "€".as_bytes();
        let text = take_in_parts(&[&euro[..1], &euro[1..2], &euro[2..]]);
        assert_eq!(text, "€");
    }

    #[test]
    fn text_before_a_split_character_is_still_flushed() {
        let mut pending = Vec::new();
        let chunk = [b'a', 0xF0, 0x9F];
        assert_eq!(take_utf8(&mut pending, &chunk), "a");
        assert_eq!(pending, [0xF0, 0x9F]);
        assert_eq!(take_utf8(&mut pending, &[0x98, 0x80]), "😀");
    }

    #[test]
    fn a_message_never_exceeds_the_chunk_ceiling() {
        let mut pending = Vec::new();
        let text = take_utf8(&mut pending, &vec![b'x'; MAX_MESSAGE + 10]);
        assert_eq!(text.len(), MAX_MESSAGE);
        assert_eq!(pending.len(), 10);
    }

    #[test]
    fn invalid_bytes_render_as_replacement_characters() {
        assert_eq!(take_in_parts(&[&[0xFF, b'o', b'k']]), "\u{FFFD}ok");
    }

    #[cfg(unix)]
    #[test]
    fn a_pty_command_reports_its_output_and_exit_code() {
        let PtyPair { slave, master } = native_pty_system().openpty(size(80, 24)).unwrap();
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.arg("-c");
        cmd.arg("printf 'hello'; exit 3");
        let mut child = slave.spawn_command(cmd).unwrap();
        drop(slave);

        let reader = master.try_clone_reader().unwrap();
        let mut output = String::new();
        pump(reader, |text| {
            output.push_str(text);
            true
        });

        assert_eq!(output, "hello");
        assert_eq!(child.wait().unwrap().exit_code(), 3);
    }
}
