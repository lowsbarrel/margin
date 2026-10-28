use std::path::Path;
use std::sync::Arc;

use margin_kernel::StoreError;

use crate::domain::TerminalSize;

pub trait PtyEvents: Send + Sync + 'static {
    fn output(&self, bytes: &[u8]);

    fn exited(&self, code: i32);
}

pub trait PtySession: Send {
    fn write(&self, data: &[u8]) -> Result<(), StoreError>;

    fn resize(&self, size: TerminalSize) -> Result<(), StoreError>;

    fn kill(&self);
}

pub trait PtyHost: Send + Sync {
    fn spawn(
        &self,
        cwd: &Path,
        size: TerminalSize,
        events: Arc<dyn PtyEvents>,
    ) -> Result<Box<dyn PtySession>, StoreError>;
}
