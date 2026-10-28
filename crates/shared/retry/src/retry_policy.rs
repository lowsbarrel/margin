use std::future::Future;
use std::time::Duration;

use margin_kernel::BoxFuture;

pub trait Retryable {
    fn is_transient(&self) -> bool;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Jitter {
    #[default]
    Full,
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub jitter: Jitter,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 4,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(5),
            jitter: Jitter::Full,
        }
    }
}

impl RetryPolicy {
    #[must_use]
    pub fn delay_for(&self, failed_attempts: u32, jitter_sample: f64) -> Duration {
        let exponent = failed_attempts.saturating_sub(1).min(31);
        let ceiling = self
            .base_delay
            .saturating_mul(1_u32 << exponent)
            .min(self.max_delay);
        match self.jitter {
            Jitter::Disabled => ceiling,
            Jitter::Full => ceiling.mul_f64(jitter_sample.clamp(0.0, 1.0).mul_add(0.5, 0.5)),
        }
    }
}

pub async fn retry<T, E, Operation, Attempt, Sleep>(
    policy: &RetryPolicy,
    sleep: Sleep,
    mut operation: Operation,
) -> Result<T, E>
where
    E: Retryable,
    Operation: FnMut() -> Attempt,
    Attempt: Future<Output = Result<T, E>>,
    Sleep: Fn(Duration) -> BoxFuture<'static, ()>,
{
    let mut failed_attempts = 0;
    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                failed_attempts += 1;
                if !error.is_transient() || failed_attempts >= policy.max_attempts.max(1) {
                    return Err(error);
                }
                sleep(policy.delay_for(failed_attempts, rand::random())).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::pin::pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, Waker};

    use parking_lot::Mutex;

    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    enum Failure {
        Transient,
        Permanent,
    }

    impl Retryable for Failure {
        fn is_transient(&self) -> bool {
            *self == Self::Transient
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let mut future = pin!(future);
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
                return value;
            }
        }
    }

    fn recorded_sleep(
        slept: &Arc<Mutex<Vec<Duration>>>,
    ) -> impl Fn(Duration) -> BoxFuture<'static, ()> {
        let slept = Arc::clone(slept);
        move |duration| {
            slept.lock().push(duration);
            Box::pin(async {})
        }
    }

    fn policy() -> RetryPolicy {
        RetryPolicy {
            jitter: Jitter::Disabled,
            ..RetryPolicy::default()
        }
    }

    #[test]
    fn delays_double_up_to_the_cap() {
        let policy = policy();
        let delays: Vec<u128> = (1..=7)
            .map(|attempt| policy.delay_for(attempt, 0.0).as_millis())
            .collect();
        assert_eq!(delays, [200, 400, 800, 1600, 3200, 5000, 5000]);
    }

    #[test]
    fn jitter_stays_within_half_to_full_delay() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.delay_for(1, 0.0), Duration::from_millis(100));
        assert_eq!(policy.delay_for(1, 1.0), Duration::from_millis(200));
        assert_eq!(policy.delay_for(1, 7.0), Duration::from_millis(200));
    }

    #[test]
    fn retries_transient_failures_until_success() {
        let slept = Arc::new(Mutex::new(Vec::new()));
        let calls = Cell::new(0);
        let result = block_on(retry(&policy(), recorded_sleep(&slept), || {
            calls.set(calls.get() + 1);
            let outcome = if calls.get() < 3 {
                Err(Failure::Transient)
            } else {
                Ok(calls.get())
            };
            async move { outcome }
        }));
        assert_eq!(result, Ok(3));
        assert_eq!(
            slept.lock().as_slice(),
            [Duration::from_millis(200), Duration::from_millis(400)]
        );
    }

    #[test]
    fn stops_at_permanent_failures_and_after_the_last_attempt() {
        let slept = Arc::new(Mutex::new(Vec::new()));
        let permanent: Result<(), Failure> =
            block_on(retry(&policy(), recorded_sleep(&slept), || async {
                Err(Failure::Permanent)
            }));
        assert_eq!(permanent, Err(Failure::Permanent));
        assert!(slept.lock().is_empty());
        let calls = Cell::new(0);
        let exhausted: Result<(), Failure> =
            block_on(retry(&policy(), recorded_sleep(&slept), || {
                calls.set(calls.get() + 1);
                async { Err(Failure::Transient) }
            }));
        assert_eq!(exhausted, Err(Failure::Transient));
        assert_eq!(calls.get(), 4);
    }
}
