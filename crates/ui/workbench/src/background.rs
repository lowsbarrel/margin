use std::fmt;
use std::future::Future;
use std::sync::Arc;

use gpui_kit::{App, Global, Task};
use margin_kernel::BoxFuture;

pub trait NetworkRuntime: Send + Sync {
    fn block_on(&self, future: BoxFuture<'_, ()>);
}

struct NetworkRuntimeGlobal(Arc<dyn NetworkRuntime>);

impl Global for NetworkRuntimeGlobal {}

impl fmt::Debug for NetworkRuntimeGlobal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NetworkRuntimeGlobal")
    }
}

pub fn install_network_runtime(runtime: Arc<dyn NetworkRuntime>, cx: &mut App) {
    cx.set_global(NetworkRuntimeGlobal(runtime));
}

pub fn run_blocking<T, Work>(cx: &App, work: Work) -> Task<T>
where
    T: Send + 'static,
    Work: FnOnce() -> T + Send + 'static,
{
    cx.background_executor().spawn(async move { work() })
}

pub fn run_async<T, Work>(cx: &App, work: Work) -> Task<T>
where
    T: Send + 'static,
    Work: Future<Output = T> + Send + 'static,
{
    let runtime = Arc::clone(&cx.global::<NetworkRuntimeGlobal>().0);
    cx.background_executor().spawn(async move {
        let mut output = None;
        runtime.block_on(Box::pin(async {
            output = Some(work.await);
        }));
        output.expect("the network runtime drives every future to completion")
    })
}
