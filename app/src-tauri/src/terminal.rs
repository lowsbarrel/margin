//! Interactive terminals: real PTYs owned by Rust, streamed to the webview.
//!
//! The frontend drives an xterm.js instance; everything that touches a file
//! descriptor happens here. Output travels over a Tauri `Channel` in chunks
//! that are always complete UTF-8 sequences, so a multi-byte character is
//! never split between two messages.

use crate::fs::VaultPathState;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtyPair, PtySize, native_pty_system};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tauri::ipc::Channel;

/// Ceiling for one IPC message. Tauri hands JSON payloads of up to 8192 bytes
/// straight to the webview through `eval`; keeping chunks at half that leaves
/// room for quoting and keeps every message off the slower `fetch` path.
const MAX_MESSAGE: usize = 4096;

pub(crate) struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

/// Live PTYs, keyed by the id the frontend allocates per terminal tab.
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

/// A PTY size, never zero: a window mid-layout can report a zero-sized box and
/// the kernel rejects it.
fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.max(1),
        cols: cols.max(1),
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// The user's own shell. `new_default_prog` resolves `$SHELL` (falling back to
/// the password database) and on unix runs it as a login shell by prefixing
/// argv0 with `-`, which is how a GUI-launched app gets the PATH that
/// `/etc/zprofile`'s `path_helper` builds — without it Homebrew is missing.
/// On Windows it resolves `%COMSPEC%`.
fn default_shell() -> CommandBuilder {
    CommandBuilder::new_default_prog()
}

/// Length of the longest prefix of `bytes` that is valid UTF-8. An error with
/// no `error_len` is an unterminated sequence at the very end of the buffer —
/// the only case held back for the next read. Bytes that are invalid in any
/// way are kept so they surface as U+FFFD rather than stalling the stream.
fn complete_prefix_len(bytes: &[u8]) -> usize {
    match std::str::from_utf8(bytes) {
        Ok(_) => bytes.len(),
        Err(err) => match err.error_len() {
            None => err.valid_up_to(),
            Some(_) => bytes.len(),
        },
    }
}

/// Append `bytes` to `pending` and take every complete UTF-8 sequence out of
/// it, capped at [`MAX_MESSAGE`]. Whatever is left is a partial character
/// waiting for its continuation bytes.
fn take_utf8(pending: &mut Vec<u8>, bytes: &[u8]) -> String {
    pending.extend_from_slice(bytes);
    let cut = complete_prefix_len(pending).min(MAX_MESSAGE);
    let text = String::from_utf8_lossy(&pending[..cut]).into_owned();
    pending.drain(..cut);
    text
}

/// Read the PTY until it closes, handing each chunk to `emit`. Returning
/// `false` from `emit` stops the pump — it is how a dead webview ends the
/// reader instead of queueing output nobody will read. `emit` is a synchronous
/// IPC hop, so the pump throttles itself: one message is ever in flight.
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
            // A read error here is the end of the stream: the child closing its
            // side makes the master return EIO on macOS.
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
    // Holding the slave open in the parent keeps the master from ever seeing
    // EOF, so the reader thread would outlive the shell.
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
    // Reusing an id must not leak the previous shell.
    drop(replaced);

    std::thread::spawn(move || pump(reader, |text| on_output.send(text.to_owned()).is_ok()));

    let owned = Arc::clone(sessions);
    std::thread::spawn(move || {
        // `wait` is also what reaps the child; without it a finished shell stays
        // a zombie until the app exits.
        let code = child.wait().map_or(-1, |status| status.exit_code() as i32);
        let _ = on_exit.send(code);
        if let Ok(mut map) = owned.lock() {
            map.remove(&id);
        }
    });

    Ok(())
}

/// Start a shell in the open vault. `on_output` streams the terminal and
/// `on_exit` fires once with the shell's exit code.
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

/// Send keystrokes to a shell. Control characters (Ctrl+C, Ctrl+D) travel this
/// way rather than as signals.
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

/// Tell the kernel (and through it the shell) that the window changed size.
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

/// Close one shell. Dropping the session closes the master, which hangs up the
/// terminal's foreground process group — the shell's own children included.
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

/// Close every shell — used when a vault is locked or the window goes away.
#[tauri::command]
#[specta::specta]
pub fn pty_kill_all(state: tauri::State<'_, TerminalState>) -> Result<(), String> {
    kill_all(&state);
    Ok(())
}

/// Drop every session. Shells are not part of the app's process tree, so
/// nothing else reaps them: without this they survive as orphans holding the
/// vault as their working directory.
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
        // "€" is E2 82 AC: the first two reads end mid-character.
        let euro = "€".as_bytes();
        let text = take_in_parts(&[&euro[..1], &euro[1..2], &euro[2..]]);
        assert_eq!(text, "€");
    }

    #[test]
    fn text_before_a_split_character_is_still_flushed() {
        let mut pending = Vec::new();
        let chunk = [b'a', 0xF0, 0x9F];
        // Leading ASCII is delivered immediately; the partial emoji is not.
        assert_eq!(take_utf8(&mut pending, &chunk), "a");
        assert_eq!(pending, [0xF0, 0x9F]);
        assert_eq!(take_utf8(&mut pending, &[0x98, 0x80]), "😀");
    }

    #[test]
    fn a_message_never_exceeds_the_chunk_ceiling() {
        let mut pending = Vec::new();
        let text = take_utf8(&mut pending, &vec![b'x'; MAX_MESSAGE + 10]);
        assert_eq!(text.len(), MAX_MESSAGE);
        // The remainder is not lost — it is waiting in the buffer.
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
        // The real pump: reading a live PTY is the behaviour under test, and the
        // child exiting is what ends it.
        pump(reader, |text| {
            output.push_str(text);
            true
        });

        assert_eq!(output, "hello");
        assert_eq!(child.wait().unwrap().exit_code(), 3);
    }
}
