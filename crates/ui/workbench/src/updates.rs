use std::fmt;
use std::sync::Arc;

use gpui_kit::{App, Global, Task};
use margin_kernel::{BoxFuture, StoreError};

use crate::background::run_async;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailableUpdate {
    pub version: String,
    pub notes: Option<String>,
}

pub trait UpdateChannel: Send + Sync {
    fn check(&self) -> BoxFuture<'static, Result<Option<AvailableUpdate>, StoreError>>;

    fn install(&self, update: AvailableUpdate) -> BoxFuture<'static, Result<(), StoreError>>;
}

#[derive(Clone)]
pub struct Updates(Arc<dyn UpdateChannel>);

impl Global for Updates {}

impl fmt::Debug for Updates {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Updates")
    }
}

impl Updates {
    pub fn install_channel(channel: Arc<dyn UpdateChannel>, cx: &mut App) {
        cx.set_global(Self(channel));
    }

    pub fn check(cx: &App) -> Task<Result<Option<AvailableUpdate>, StoreError>> {
        let future = cx.global::<Self>().0.check();
        run_async(cx, future)
    }

    pub fn install(update: AvailableUpdate, cx: &App) -> Task<Result<(), StoreError>> {
        let future = cx.global::<Self>().0.install(update);
        run_async(cx, future)
    }
}
