use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

use parking_lot::Mutex;

use crate::domain::{SessionId, TerminalSize};
use crate::error::Error;
use crate::ports::{PtyEvents, PtyHost, PtySession};

#[derive(Clone)]
pub struct Terminals {
    inner: Arc<Inner>,
}

struct Inner {
    host: Arc<dyn PtyHost>,
    generations: AtomicU64,
    sessions: Mutex<HashMap<SessionId, Running>>,
}

struct Running {
    generation: u64,
    session: Box<dyn PtySession>,
}

struct SessionSink {
    id: SessionId,
    generation: u64,
    owner: Weak<Inner>,
    events: Arc<dyn PtyEvents>,
}

impl Terminals {
    #[must_use]
    pub fn new(host: Arc<dyn PtyHost>) -> Self {
        Self {
            inner: Arc::new(Inner {
                host,
                generations: AtomicU64::new(0),
                sessions: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn spawn(
        &self,
        id: SessionId,
        cwd: &Path,
        size: TerminalSize,
        events: Arc<dyn PtyEvents>,
    ) -> Result<(), Error> {
        if cwd.as_os_str().is_empty() {
            return Err(Error::NoVault);
        }
        let generation = self.inner.generations.fetch_add(1, Ordering::Relaxed);
        let sink = Arc::new(SessionSink {
            id,
            generation,
            owner: Arc::downgrade(&self.inner),
            events,
        });
        let mut sessions = self.inner.sessions.lock();
        let session = self.inner.host.spawn(cwd, size, sink)?;
        if let Some(replaced) = sessions.insert(
            id,
            Running {
                generation,
                session,
            },
        ) {
            replaced.session.kill();
        }
        Ok(())
    }

    pub fn write(&self, id: SessionId, data: &[u8]) -> Result<(), Error> {
        let sessions = self.inner.sessions.lock();
        let running = sessions.get(&id).ok_or(Error::NotRunning(id))?;
        running.session.write(data)?;
        Ok(())
    }

    pub fn resize(&self, id: SessionId, size: TerminalSize) -> Result<(), Error> {
        let sessions = self.inner.sessions.lock();
        let running = sessions.get(&id).ok_or(Error::NotRunning(id))?;
        running.session.resize(size)?;
        Ok(())
    }

    pub fn kill(&self, id: SessionId) {
        let running = self.inner.sessions.lock().remove(&id);
        if let Some(running) = running {
            running.session.kill();
        }
    }

    pub fn kill_all(&self) {
        let mut sessions = self.inner.sessions.lock();
        for (_, running) in sessions.drain() {
            running.session.kill();
        }
    }
}

impl fmt::Debug for Terminals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Terminals").finish_non_exhaustive()
    }
}

impl Inner {
    fn remove(&self, id: SessionId, generation: u64) {
        let mut sessions = self.sessions.lock();
        let matches = sessions
            .get(&id)
            .is_some_and(|running| running.generation == generation);
        if matches {
            sessions.remove(&id);
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        for (_, running) in self.sessions.get_mut().drain() {
            running.session.kill();
        }
    }
}

impl PtyEvents for SessionSink {
    fn output(&self, bytes: &[u8]) {
        self.events.output(bytes);
    }

    fn exited(&self, code: i32) {
        self.events.exited(code);
        if let Some(inner) = self.owner.upgrade() {
            inner.remove(self.id, self.generation);
        }
    }
}
