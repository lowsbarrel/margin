use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures_util::StreamExt;
use gpui_kit::App;
use margin_terminals::{PtyEvents, SessionId, TerminalSize, Terminals};
use parking_lot::{Mutex, MutexGuard};

use crate::emulator::Emulator;

#[derive(Debug)]
pub(crate) enum Wake {
    Output,
    Exited(i32),
    Failed(String),
}

pub(crate) struct Screen {
    emulator: Mutex<Emulator>,
    output_pending: AtomicBool,
    wake: UnboundedSender<Wake>,
}

impl fmt::Debug for Screen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Screen").finish_non_exhaustive()
    }
}

impl Screen {
    pub(crate) fn new(size: TerminalSize) -> (Arc<Self>, UnboundedReceiver<Wake>) {
        let (wake, woken) = mpsc::unbounded();
        let screen = Arc::new(Self {
            emulator: Mutex::new(Emulator::new(size)),
            output_pending: AtomicBool::new(false),
            wake,
        });
        (screen, woken)
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Emulator> {
        self.emulator.lock()
    }

    pub(crate) fn output_seen(&self) {
        self.output_pending.store(false, Ordering::Release);
    }

    fn send(&self, wake: Wake) {
        drop(self.wake.unbounded_send(wake));
    }
}

impl PtyEvents for Screen {
    fn output(&self, bytes: &[u8]) {
        self.emulator.lock().advance(bytes);
        if !self.output_pending.swap(true, Ordering::AcqRel) {
            self.send(Wake::Output);
        }
    }

    fn exited(&self, code: i32) {
        self.send(Wake::Exited(code));
    }
}

#[derive(Debug)]
enum Command {
    Write(Vec<u8>),
    Resize(TerminalSize),
}

#[derive(Debug)]
pub(crate) struct Session {
    commands: UnboundedSender<Command>,
}

impl Session {
    pub(crate) fn start(
        id: SessionId,
        terminals: Terminals,
        cwd: PathBuf,
        size: TerminalSize,
        screen: Arc<Screen>,
        cx: &App,
    ) -> Self {
        let (commands, mut pending) = mpsc::unbounded();
        cx.background_executor()
            .spawn(async move {
                let shared: Arc<Screen> = Arc::clone(&screen);
                let events: Arc<dyn PtyEvents> = shared;
                if let Err(error) = terminals.spawn(id, &cwd, size, events) {
                    screen.send(Wake::Failed(error.to_string()));
                    return;
                }
                while let Some(command) = pending.next().await {
                    let result = match command {
                        Command::Write(bytes) => terminals.write(id, &bytes),
                        Command::Resize(size) => terminals.resize(id, size),
                    };
                    drop(result);
                }
                terminals.kill(id);
            })
            .detach();
        Self { commands }
    }

    pub(crate) fn write(&self, bytes: Vec<u8>) {
        drop(self.commands.unbounded_send(Command::Write(bytes)));
    }

    pub(crate) fn resize(&self, size: TerminalSize) {
        drop(self.commands.unbounded_send(Command::Resize(size)));
    }
}
