use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use margin_kernel::StoreError;
use margin_terminals::{PtyEvents, PtyHost, PtySession, TerminalSize};
use parking_lot::Mutex;

#[derive(Clone, Debug, Default)]
pub struct FakePtyHost {
    state: Arc<Mutex<FakeState>>,
}

#[derive(Debug, Default)]
struct FakeState {
    sessions: Vec<FakeSession>,
    failure: Option<StoreError>,
}

#[derive(Clone)]
pub struct FakeSession {
    cwd: PathBuf,
    size: TerminalSize,
    events: Arc<dyn PtyEvents>,
    written: Arc<Mutex<Vec<u8>>>,
    resizes: Arc<Mutex<Vec<TerminalSize>>>,
    killed: Arc<AtomicBool>,
}

impl FakePtyHost {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fail_next_spawn(&self, error: StoreError) {
        self.state.lock().failure = Some(error);
    }

    #[must_use]
    pub fn session(&self, index: usize) -> Option<FakeSession> {
        self.state.lock().sessions.get(index).cloned()
    }

    #[must_use]
    pub fn last_session(&self) -> Option<FakeSession> {
        self.state.lock().sessions.last().cloned()
    }

    #[must_use]
    pub fn session_count(&self) -> usize {
        self.state.lock().sessions.len()
    }
}

impl PtyHost for FakePtyHost {
    fn spawn(
        &self,
        cwd: &Path,
        size: TerminalSize,
        events: Arc<dyn PtyEvents>,
    ) -> Result<Box<dyn PtySession>, StoreError> {
        let mut state = self.state.lock();
        if let Some(error) = state.failure.take() {
            return Err(error);
        }
        let session = FakeSession {
            cwd: cwd.to_path_buf(),
            size,
            events,
            written: Arc::new(Mutex::new(Vec::new())),
            resizes: Arc::new(Mutex::new(Vec::new())),
            killed: Arc::new(AtomicBool::new(false)),
        };
        state.sessions.push(session.clone());
        Ok(Box::new(session))
    }
}

impl FakeSession {
    #[must_use]
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    #[must_use]
    pub fn size(&self) -> TerminalSize {
        self.size
    }

    #[must_use]
    pub fn written(&self) -> Vec<u8> {
        self.written.lock().clone()
    }

    #[must_use]
    pub fn resizes(&self) -> Vec<TerminalSize> {
        self.resizes.lock().clone()
    }

    #[must_use]
    pub fn killed(&self) -> bool {
        self.killed.load(Ordering::Relaxed)
    }

    pub fn output(&self, bytes: &[u8]) {
        self.events.output(bytes);
    }

    pub fn exit(&self, code: i32) {
        self.events.exited(code);
    }
}

impl fmt::Debug for FakeSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeSession")
            .field("cwd", &self.cwd)
            .field("size", &self.size)
            .field("killed", &self.killed())
            .finish_non_exhaustive()
    }
}

impl PtySession for FakeSession {
    fn write(&self, data: &[u8]) -> Result<(), StoreError> {
        self.written.lock().extend_from_slice(data);
        Ok(())
    }

    fn resize(&self, size: TerminalSize) -> Result<(), StoreError> {
        self.resizes.lock().push(size);
        Ok(())
    }

    fn kill(&self) {
        self.killed.store(true, Ordering::Relaxed);
    }
}
