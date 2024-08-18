#![doc = include_str!("../README.md")]

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

#[doc(hidden)]
pub struct RateLimiter {
    count: usize,
    timestamp: Instant,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            count: 0,
            timestamp: Instant::now(),
        }
    }

    pub fn log_maybe(&mut self, period: Duration, max_per_time: usize, log: impl Fn()) {
        if self.count < max_per_time {
            log();
            if self.count == max_per_time - 1 {
                log::warn!(
                    "Starting to ignore the previous log for less than {:?}",
                    period
                );
            }
        } else {
            let now = Instant::now();
            if now.duration_since(self.timestamp) > period {
                let filtered_log_count = self.count - max_per_time;
                if filtered_log_count > 0 {
                    log::warn!(
                    "Ignored {filtered_log_count} logs since more than {:?} ago. Starting again...",
                    period
                );
                }
                log();
                self.timestamp = now;
            }
        }

        self.count += 1;
    }
}

#[doc(hidden)]
pub struct SynchronisedRateLimiter {
    count: AtomicUsize,
    timestamp: LazyLock<Mutex<Instant>>,
}

impl Default for SynchronisedRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl SynchronisedRateLimiter {
    pub const fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
            timestamp: LazyLock::new(|| Instant::now().into()),
        }
    }

    pub fn log_maybe(&self, period: Duration, max_per_time: usize, log: impl Fn()) {
        let count = self.count.fetch_add(1, Ordering::Relaxed);
        if count < max_per_time {
            log();
            if count == max_per_time - 1 {
                log::warn!(
                    "Starting to ignore the previous log for less than {:?}",
                    period
                );
            }
        } else {
            let now = Instant::now();
            let mut timestamp = self.timestamp.lock().unwrap();
            if now.duration_since(*timestamp) > period {
                let filtered_log_count = self.count.swap(1, Ordering::Relaxed) - max_per_time;
                if filtered_log_count > 0 {
                    log::warn!(
                    "Ignored {filtered_log_count} logs since more than {:?} ago. Starting again...",
                    period
                );
                }
                log();
                *timestamp = now;
            }
        }
    }
}

// TODO: Write a macro to dedup this
#[macro_export]
macro_rules! error_limit {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::SynchronisedRateLimiter;
        static RATE_LIMITER: SynchronisedRateLimiter = SynchronisedRateLimiter::new();
        RATE_LIMITER.log_maybe($period, $max_per_time, || log::log!(log::Level::Error, $($arg)+));
    }};
}

#[macro_export]
macro_rules! warn_limit {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::SynchronisedRateLimiter;
        static RATE_LIMITER: SynchronisedRateLimiter = SynchronisedRateLimiter::new();
        RATE_LIMITER.log_maybe($period, $max_per_time, || log::log!(log::Level::Warn, $($arg)+));
    }};
}

#[macro_export]
macro_rules! info_limit {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::SynchronisedRateLimiter;
        static RATE_LIMITER: SynchronisedRateLimiter = SynchronisedRateLimiter::new();
        RATE_LIMITER.log_maybe($period, $max_per_time, || log::log!(log::Level::Info, $($arg)+));
    }};
}

#[macro_export]
macro_rules! debug_limit {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::SynchronisedRateLimiter;
        static RATE_LIMITER: SynchronisedRateLimiter = SynchronisedRateLimiter::new();
        RATE_LIMITER.log_maybe($period, $max_per_time, || log::log!(log::Level::Debug, $($arg)+));
    }};
}

#[macro_export]
macro_rules! trace_limit {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::SynchronisedRateLimiter;
        static RATE_LIMITER: SynchronisedRateLimiter = SynchronisedRateLimiter::new();
        RATE_LIMITER.log_maybe($period, $max_per_time, || log::log!(log::Level::Trace, $($arg)+));
    }};
}

#[macro_export]
macro_rules! error_limit_thread {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::RateLimiter;
        use std::cell::RefCell;
        use std::thread_local;

        thread_local! {
            static RATE_LIMITER: RefCell<RateLimiter> = RefCell::new(RateLimiter::new());
        }

        RATE_LIMITER.with(|rate_limiter| {
            rate_limiter
                .borrow_mut()
                .log_maybe($period, $max_per_time, || log::log!(log::Level::Error, $($arg)+))
        });
    }};
}

#[macro_export]
macro_rules! warn_limit_thread {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::RateLimiter;
        use std::cell::RefCell;
        use std::thread_local;

        thread_local! {
            static RATE_LIMITER: RefCell<RateLimiter> = RefCell::new(RateLimiter::new());
        }

        RATE_LIMITER.with(|rate_limiter| {
            rate_limiter
                .borrow_mut()
                .log_maybe($period, $max_per_time, || log::log!(log::Level::Warn, $($arg)+))
        });
    }};
}

#[macro_export]
macro_rules! info_limit_thread {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::RateLimiter;
        use std::cell::RefCell;
        use std::thread_local;

        thread_local! {
            static RATE_LIMITER: RefCell<RateLimiter> = RefCell::new(RateLimiter::new());
        }

        RATE_LIMITER.with(|rate_limiter| {
            rate_limiter
                .borrow_mut()
                .log_maybe($period, $max_per_time, || log::log!(log::Level::Info, $($arg)+))
        });
    }};
}

#[macro_export]
macro_rules! debug_limit_thread {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::RateLimiter;
        use std::cell::RefCell;
        use std::thread_local;

        thread_local! {
            static RATE_LIMITER: RefCell<RateLimiter> = RefCell::new(RateLimiter::new());
        }

        RATE_LIMITER.with(|rate_limiter| {
            rate_limiter
                .borrow_mut()
                .log_maybe($period, $max_per_time, || log::log!(log::Level::Debug, $($arg)+))
        });
    }};
}

#[macro_export]
macro_rules! trace_limit_thread {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::RateLimiter;
        use std::cell::RefCell;
        use std::thread_local;

        thread_local! {
            static RATE_LIMITER: RefCell<RateLimiter> = RefCell::new(RateLimiter::new());
        }

        RATE_LIMITER.with(|rate_limiter| {
            rate_limiter
                .borrow_mut()
                .log_maybe($period, $max_per_time, || log::log!(log::Level::Trace, $($arg)+))
        });
    }};
}

#[cfg(test)]
mod tests {
    use super::info_limit;
    use std::thread;
    use std::time::Duration;

    use simple_logger::SimpleLogger;

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
    }

    #[test]
    fn all_synchronised_variants_compile() {
        error_limit!(1, Duration::from_millis(1), "");
        warn_limit!(1, Duration::from_millis(1), "");
        info_limit!(1, Duration::from_millis(1), "");
        debug_limit!(1, Duration::from_millis(1), "");
        trace_limit!(1, Duration::from_millis(1), "");
    }

    #[test]
    fn all_thread_variants_compile() {
        error_limit_thread!(1, Duration::from_millis(1), "");
        warn_limit_thread!(1, Duration::from_millis(1), "");
        info_limit_thread!(1, Duration::from_millis(1), "");
        debug_limit_thread!(1, Duration::from_millis(1), "");
        trace_limit_thread!(1, Duration::from_millis(1), "");
    }
}
