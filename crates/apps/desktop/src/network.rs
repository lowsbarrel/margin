use std::time::Duration;

use margin_kernel::{BoxFuture, Timer};
use margin_workbench::NetworkRuntime;
use tokio::runtime::{Builder, Runtime};
use tokio::time;

#[derive(Debug)]
pub(crate) struct TokioNetwork {
    runtime: Runtime,
}

impl TokioNetwork {
    pub(crate) fn start() -> Self {
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("margin-network")
            .enable_all()
            .build()
            .expect("the operating system provides threads and timers for the network runtime");
        Self { runtime }
    }
}

impl NetworkRuntime for TokioNetwork {
    fn block_on(&self, future: BoxFuture<'_, ()>) {
        self.runtime.block_on(future);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TokioTimer;

impl Timer for TokioTimer {
    fn sleep(&self, duration: Duration) -> BoxFuture<'static, ()> {
        Box::pin(time::sleep(duration))
    }
}
