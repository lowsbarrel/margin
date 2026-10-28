use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{env, thread};

use margin_kernel::StoreError;
use margin_terminals::{PtyEvents, PtyHost, PtySession, TerminalSize};
use parking_lot::Mutex;
use portable_pty::{
    ChildKiller, CommandBuilder, ExitStatus, MasterPty, PtyPair, PtySize, native_pty_system,
};

const READ_BUFFER: usize = 8192;

#[derive(Debug, Default)]
pub struct NativePtyHost;

impl PtyHost for NativePtyHost {
    fn spawn(
        &self,
        cwd: &Path,
        size: TerminalSize,
        events: Arc<dyn PtyEvents>,
    ) -> Result<Box<dyn PtySession>, StoreError> {
        open(default_shell(cwd), size, events)
    }
}

fn default_shell(cwd: &Path) -> CommandBuilder {
    let mut command = CommandBuilder::new_default_prog();
    command.cwd(cwd);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    if env::var_os("LANG").is_none() {
        command.env("LANG", "en_US.UTF-8");
    }
    command
}

fn open(
    command: CommandBuilder,
    size: TerminalSize,
    events: Arc<dyn PtyEvents>,
) -> Result<Box<dyn PtySession>, StoreError> {
    let PtyPair { slave, master } =
        native_pty_system()
            .openpty(pty_size(size))
            .map_err(|error| {
                StoreError::Unavailable(format!("failed to open a pseudo-terminal: {error}"))
            })?;
    let mut child = slave
        .spawn_command(command)
        .map_err(|error| StoreError::Unavailable(format!("failed to start the shell: {error}")))?;
    drop(slave);
    let reader = master.try_clone_reader().map_err(|error| {
        StoreError::Unavailable(format!("failed to read from the terminal: {error}"))
    })?;
    let writer = master.take_writer().map_err(|error| {
        StoreError::Unavailable(format!("failed to write to the terminal: {error}"))
    })?;
    let killer = child.clone_killer();

    let (sender, receiver) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || forward_writes(writer, &receiver));
    let output_events = Arc::clone(&events);
    thread::spawn(move || forward_output(reader, output_events.as_ref()));
    thread::spawn(move || {
        let code = child.wait().map_or(-1, |status| exit_code(&status));
        events.exited(code);
    });

    Ok(Box::new(Session {
        master,
        writer: sender,
        killer: Mutex::new(killer),
    }))
}

fn forward_writes(mut writer: Box<dyn Write + Send>, receiver: &Receiver<Vec<u8>>) {
    while let Ok(data) = receiver.recv() {
        if writer
            .write_all(&data)
            .and_then(|()| writer.flush())
            .is_err()
        {
            break;
        }
    }
}

fn forward_output(mut reader: Box<dyn Read + Send>, events: &dyn PtyEvents) {
    let mut buffer = [0_u8; READ_BUFFER];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => events.output(&buffer[..read]),
            Err(error) => {
                if error.kind() != io::ErrorKind::Interrupted {
                    break;
                }
            }
        }
    }
}

fn exit_code(status: &ExitStatus) -> i32 {
    i32::try_from(status.exit_code()).unwrap_or(-1)
}

const fn pty_size(size: TerminalSize) -> PtySize {
    PtySize {
        rows: size.rows(),
        cols: size.cols(),
        pixel_width: 0,
        pixel_height: 0,
    }
}

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Sender<Vec<u8>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

impl PtySession for Session {
    fn write(&self, data: &[u8]) -> Result<(), StoreError> {
        self.writer.send(data.to_vec()).map_err(|error| {
            StoreError::Unavailable(format!("the terminal stopped accepting input: {error}"))
        })
    }

    fn resize(&self, size: TerminalSize) -> Result<(), StoreError> {
        self.master.resize(pty_size(size)).map_err(|error| {
            StoreError::Unavailable(format!("failed to resize the terminal: {error}"))
        })
    }

    fn kill(&self) {
        self.killer.lock().kill().unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::{OsStr, OsString};
    use std::time::Duration;

    use margin_test_support::RecordingPtyEvents;

    use super::*;

    const TIMEOUT: Duration = Duration::from_secs(10);

    #[test]
    fn the_default_shell_starts_in_the_given_directory_with_a_terminal_environment() {
        let command = default_shell(Path::new("/vault"));
        assert_eq!(
            command.get_cwd().map(OsString::as_os_str),
            Some(OsStr::new("/vault"))
        );
        assert_eq!(command.get_env("TERM"), Some(OsStr::new("xterm-256color")));
        assert_eq!(command.get_env("COLORTERM"), Some(OsStr::new("truecolor")));
    }

    #[test]
    fn a_command_reports_its_output_and_exit_code() {
        let events = Arc::new(RecordingPtyEvents::new());
        let session = open(
            echo_command(),
            TerminalSize::new(80, 24),
            Arc::<RecordingPtyEvents>::clone(&events),
        )
        .expect("the command starts in a pseudo-terminal");
        #[cfg(windows)]
        answer_cursor_position_query(session.as_ref(), &events);
        assert!(
            events.wait_for_output("hi", TIMEOUT),
            "the command writes its output: {:?}",
            events.output_bytes()
        );
        assert_eq!(events.wait_for_exit(TIMEOUT), Some(0));
        drop(session);
    }

    #[cfg(windows)]
    fn answer_cursor_position_query(session: &dyn PtySession, events: &RecordingPtyEvents) {
        assert!(
            events.wait_for_output("\u{1b}[6n", TIMEOUT),
            "the pseudo-console asks for the cursor position"
        );
        session
            .write(b"\x1b[1;1R")
            .expect("the pseudo-console accepts the cursor position report");
    }

    #[cfg(windows)]
    fn echo_command() -> CommandBuilder {
        let mut command = CommandBuilder::new("cmd");
        command.arg("/C");
        command.arg("echo hi");
        command
    }

    #[cfg(not(windows))]
    fn echo_command() -> CommandBuilder {
        let mut command = CommandBuilder::new("sh");
        command.arg("-c");
        command.arg("echo hi");
        command
    }
}
