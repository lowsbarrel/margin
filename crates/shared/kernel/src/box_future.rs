use std::future::Future;
use std::pin::Pin;

pub type BoxFuture<'future, T> = Pin<Box<dyn Future<Output = T> + Send + 'future>>;
