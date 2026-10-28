use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use margin_terminals::PtyEvents;
use parking_lot::Mutex;

const POLL_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Clone, Debug, Default)]
pub struct RecordingPtyEvents {
    output: Arc<Mutex<Vec<u8>>>,
    exits: Arc<Mutex<Vec<i32>>>,
}

impl RecordingPtyEvents {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn output_bytes(&self) -> Vec<u8> {
        self.output.lock().clone()
    }

    #[must_use]
    pub fn exit_codes(&self) -> Vec<i32> {
        self.exits.lock().clone()
    }

    #[must_use]
    pub fn wait_for_output(&self, expected: &str, timeout: Duration) -> bool {
        poll(timeout, || {
            String::from_utf8_lossy(&self.output_bytes())
                .contains(expected)
                .then_some(())
        })
        .is_some()
    }

    #[must_use]
    pub fn wait_for_exit(&self, timeout: Duration) -> Option<i32> {
        poll(timeout, || self.exits.lock().first().copied())
    }
}

impl PtyEvents for RecordingPtyEvents {
    fn output(&self, bytes: &[u8]) {
        self.output.lock().extend_from_slice(bytes);
    }

    fn exited(&self, code: i32) {
        self.exits.lock().push(code);
    }
}

fn poll<T>(timeout: Duration, mut probe: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(value) = probe() {
            return Some(value);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(POLL_INTERVAL);
    }
}
