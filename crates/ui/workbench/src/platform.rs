use std::fmt;
use std::path::Path;
use std::sync::Arc;

use gpui_kit::{App, Global};
use margin_kernel::StoreError;

type RevealHandler = Arc<dyn Fn(&Path) -> Result<(), StoreError> + Send + Sync>;
type OpenHandler = Arc<dyn Fn(&str) -> Result<(), StoreError> + Send + Sync>;

#[derive(Clone)]
pub struct Platform {
    reveal: RevealHandler,
    open_external: OpenHandler,
}

impl Global for Platform {}

impl fmt::Debug for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Platform")
    }
}

impl Platform {
    pub fn install(reveal: RevealHandler, open_external: OpenHandler, cx: &mut App) {
        cx.set_global(Self {
            reveal,
            open_external,
        });
    }

    pub fn reveal(cx: &App, path: &Path) -> Result<(), StoreError> {
        (cx.global::<Self>().reveal)(path)
    }

    pub fn open_external(cx: &App, target: &str) -> Result<(), StoreError> {
        (cx.global::<Self>().open_external)(target)
    }
}
