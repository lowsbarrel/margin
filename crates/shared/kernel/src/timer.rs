use std::time::Duration;

use crate::BoxFuture;

pub trait Timer: Send + Sync {
    fn sleep(&self, duration: Duration) -> BoxFuture<'static, ()>;
}
