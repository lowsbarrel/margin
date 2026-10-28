use crate::UnixMillis;

pub trait Clock: Send + Sync {
    fn now(&self) -> UnixMillis;
}
