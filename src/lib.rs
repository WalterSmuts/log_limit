use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

pub struct RateLimiter {
    count: AtomicUsize,
    timestamp: Mutex<Option<Instant>>,
    period: Duration,
}

impl RateLimiter {
    pub const fn new(period: Duration) -> Self {
        Self {
            count: AtomicUsize::new(0),
            timestamp: Mutex::new(None),
            period,
        }
    }

    // TODO: Find a better way to initialize this
    pub fn ensure_timestamp_init(&self) {
        let mut maybe_timestamp = self.timestamp.lock().unwrap();
        if maybe_timestamp.is_none() {
            *maybe_timestamp = Some(Instant::now());
        }
    }

    pub fn log_maybe(&self, max_per_time: usize, log: impl Fn()) {
        let count = self.count.fetch_add(1, Ordering::Relaxed);
        if count < max_per_time {
            log();
            if count == max_per_time - 1 {
                log::warn!("Starting to ignore the previous log for {:?}", self.period);
            }
        } else {
            let now = Instant::now();
            // Safe to unwrap here because we will never panic *touch wood*
            let mut maybe_timestamp = self.timestamp.lock().unwrap();
            // Safe to unwap here because we always populate with Some above if there is a none and
            // we never initialize with a none.
            if now.duration_since(maybe_timestamp.unwrap()) > self.period {
                let filtered_log_count = self.count.swap(1, Ordering::Relaxed) - max_per_time;
                log::warn!(
                    "Ignored {filtered_log_count} logs since more than {:?} ago. Starting again...",
                    self.period
                );
                log();
                *maybe_timestamp = Some(now);
            }
        }
    }
}

#[macro_export]
macro_rules! info_limit {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        static RATE_LIMITER: RateLimiter = RateLimiter::new($period);
        RATE_LIMITER.ensure_timestamp_init();
        RATE_LIMITER.log_maybe($max_per_time, || log::info!($($arg)+));
    }};
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use simple_logger::SimpleLogger;

    use super::*;

    #[test]
    fn it_works() {
        SimpleLogger::new().init().unwrap();
        let a = 1;
        info_limit!(1, Duration::from_millis(60), "Logging with nothing");
        info_limit!(
            10,
            Duration::from_millis(60),
            "Logging with arg value {}",
            10
        );
        info_limit!(
            1,
            Duration::from_millis(60),
            "Logging with inner string value {a}"
        );

        for i in 0..10 {
            log::debug!("{i}");
            info_limit!(3, Duration::from_millis(5), "Logging on repeat");
            thread::sleep(Duration::from_millis(1));
        }
        panic!()
    }
}
