#![doc = include_str!("../README.md")]

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

#[cfg(test)]
mod testing_logger;

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
        let now = Instant::now();

        #[cfg(feature = "warning-messages")]
        let calculated_duration = now.duration_since(self.timestamp);
        if self.count < max_per_time {
            log();
            self.count += 1;

            #[cfg(feature = "warning-messages")]
            if self.count == max_per_time {
                log::warn!(
                    "Hit logging threashold! Starting to ignore the previous log for {:?}",
                    calculated_duration
                );
            }
        } else {
            let calculated_duration = now.duration_since(self.timestamp);
            if calculated_duration > period {
                #[cfg(feature = "warning-messages")]
                let filtered_log_count = self.count - max_per_time;
                #[cfg(feature = "warning-messages")]
                if filtered_log_count > 0 {
                    log::warn!(
                        "Ignored {filtered_log_count} logs since {:?} ago. Starting to log again...",
                        calculated_duration
                    );
                }
                log();
                self.count = 1;
                self.timestamp = now;
            } else {
                self.count += 1;
            }
        }
    }
}

#[doc(hidden)]
pub struct SynchronisedRateLimiter {
    count: AtomicUsize,
    timestamp: Mutex<Instant>,
}

impl SynchronisedRateLimiter {
    pub const fn new() -> LazyLock<Self> {
        LazyLock::new(|| Self {
            count: AtomicUsize::new(0),
            timestamp: Instant::now().into(),
        })
    }

    pub fn log_maybe(&self, period: Duration, max_per_time: usize, log: impl Fn()) {
        let count = self.count.fetch_add(1, Ordering::Relaxed) + 1;
        if count <= max_per_time {
            log();
            #[cfg(feature = "warning-messages")]
            if count == max_per_time {
                log::warn!(
                    "Hit logging threashold! Starting to ignore the previous log for more than {:?}",
                    period
                );
            }
        } else {
            let now = Instant::now();
            let mut timestamp = self.timestamp.lock().unwrap();

            let calculated_duration = now.duration_since(*timestamp);
            if calculated_duration > period {
                #[cfg(feature = "warning-messages")]
                let filtered_log_count = self.count.swap(1, Ordering::Relaxed) - max_per_time - 1;
                #[cfg(not(feature = "warning-messages"))]
                let _filtered_log_count = self.count.swap(1, Ordering::Relaxed) - max_per_time - 1;
                #[cfg(feature = "warning-messages")]
                if filtered_log_count > 0 {
                    log::warn!(
                        "Ignored {filtered_log_count} logs since {:?} ago. Starting to log again...",
                        calculated_duration
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
macro_rules! error_limit_global {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::SynchronisedRateLimiter;
        use std::sync::LazyLock;
        static RATE_LIMITER: LazyLock<SynchronisedRateLimiter> = SynchronisedRateLimiter::new();
        RATE_LIMITER.log_maybe($period, $max_per_time, || log::log!(log::Level::Error, $($arg)+));
    }};
}

#[macro_export]
macro_rules! warn_limit_global {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::SynchronisedRateLimiter;
        use std::sync::LazyLock;
        static RATE_LIMITER: LazyLock<SynchronisedRateLimiter> = SynchronisedRateLimiter::new();
        RATE_LIMITER.log_maybe($period, $max_per_time, || log::log!(log::Level::Warn, $($arg)+));
    }};
}

#[macro_export]
macro_rules! info_limit_global {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::SynchronisedRateLimiter;
        use std::sync::LazyLock;
        static RATE_LIMITER: LazyLock<SynchronisedRateLimiter> = SynchronisedRateLimiter::new();
        RATE_LIMITER.log_maybe($period, $max_per_time, || log::log!(log::Level::Info, $($arg)+));
    }};
}

#[macro_export]
macro_rules! debug_limit_global {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::SynchronisedRateLimiter;
        use std::sync::LazyLock;
        static RATE_LIMITER: LazyLock<SynchronisedRateLimiter> = SynchronisedRateLimiter::new();
        RATE_LIMITER.log_maybe($period, $max_per_time, || log::log!(log::Level::Debug, $($arg)+));
    }};
}

#[macro_export]
macro_rules! trace_limit_global {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::SynchronisedRateLimiter;
        use std::sync::LazyLock;
        static RATE_LIMITER: LazyLock<SynchronisedRateLimiter> = SynchronisedRateLimiter::new();
        RATE_LIMITER.log_maybe($period, $max_per_time, || log::log!(log::Level::Trace, $($arg)+));
    }};
}

#[macro_export]
macro_rules! error_limit {
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
macro_rules! warn_limit {
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
macro_rules! info_limit {
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
macro_rules! debug_limit {
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
macro_rules! trace_limit {
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
    use super::info_limit_global;
    use std::thread;
    use std::time::Duration;
    use std::time::Instant;

    enum LoggerVariant {
        TheadLocal,
        Shared,
    }

    fn logger_limits_correctly(variant_under_test: LoggerVariant) {
        crate::testing_logger::setup();
        for _ in 0..11 {
            match variant_under_test {
                LoggerVariant::TheadLocal => {
                    info_limit!(2, Duration::from_millis(50), "Logging on repeat")
                }
                LoggerVariant::Shared => {
                    info_limit_global!(2, Duration::from_millis(50), "Logging on repeat")
                }
            }
            thread::sleep(Duration::from_millis(11));
            // 00: Log
            // 11: Log (and warn of omission)
            // 22: Ommit
            // 33: Ommit
            // 44: Ommit
            // 55: Log (and warn: missed 3)
            // 66: Log (and warn of omission)
            // 77: Ommit
            // 88: Ommit
            // 99: Ommit
            // 110: Log (and warn: missed 3)
        }
        crate::testing_logger::validate(|captured_logs| {
            #[cfg(feature = "warning-messages")]
            let warning_logs = captured_logs
                .iter()
                .filter(|log| log.level == log::Level::Warn);

            let info_logs = captured_logs
                .iter()
                .filter(|log| log.level == log::Level::Info);

            #[cfg(feature = "warning-messages")]
            assert_eq!(warning_logs.clone().count(), 4);
            assert_eq!(info_logs.count(), 5);
            #[cfg(feature = "warning-messages")]
            {
                let ignored_warnings: Vec<_> = warning_logs
                    .filter(|log| log.body.contains("Ignored"))
                    .collect();
                assert_eq!(ignored_warnings.len(), 2);
                assert_eq!(
                    "3",
                    ignored_warnings[0].body.split_whitespace().nth(1).unwrap()
                );
                assert_eq!(
                    "3",
                    ignored_warnings[1].body.split_whitespace().nth(1).unwrap()
                );
            }
        })
    }

    #[test]
    fn thread_local_logger_limits_correctly() {
        logger_limits_correctly(LoggerVariant::TheadLocal);
    }

    #[test]
    fn shared_logger_limits_correctly() {
        logger_limits_correctly(LoggerVariant::Shared);
    }

    const ACCEPTABLE_DROP_FACTOR: f64 = 0.99;
    const TEST_TIME_MS: usize = 500;
    const TEST_PERIOD_MS: usize = 1;
    const MAX_LOGS_PER_PERIOD: usize = 500;
    #[cfg(feature = "warning-messages")]
    const MAX_EXPECTED_WARNING_LOGS_PER_PERIOD: usize = 2;
    fn spamming_does_not_work(spam_logs: impl Fn()) {
        crate::testing_logger::setup();
        spam_logs();
        crate::testing_logger::validate(|captured_logs| {
            #[cfg(feature = "warning-messages")]
            let warning_logs = captured_logs
                .iter()
                .filter(|log| log.level == log::Level::Warn);

            let info_logs = captured_logs
                .iter()
                .filter(|log| log.level == log::Level::Info);

            #[cfg(feature = "warning-messages")]
            let warning_logs_count = warning_logs.count();
            let info_logs_count = info_logs.count();

            // Enusre we don't overstep the limit on average
            #[cfg(feature = "warning-messages")]
            assert!(
                warning_logs_count
                    <= TEST_TIME_MS / TEST_PERIOD_MS * MAX_EXPECTED_WARNING_LOGS_PER_PERIOD
            );
            assert!(info_logs_count <= MAX_LOGS_PER_PERIOD * TEST_TIME_MS);

            #[cfg(feature = "warning-messages")]
            assert!(
                warning_logs_count as f64
                    > ((TEST_TIME_MS / TEST_PERIOD_MS * MAX_EXPECTED_WARNING_LOGS_PER_PERIOD)
                        as f64
                        * ACCEPTABLE_DROP_FACTOR)
            );
            assert!(
                info_logs_count as f64
                    > ((MAX_LOGS_PER_PERIOD * TEST_TIME_MS) as f64 * ACCEPTABLE_DROP_FACTOR)
            );
        })
    }

    #[test]
    fn thread_local_spamming_does_not_work() {
        spamming_does_not_work(|| {
            let start = Instant::now();
            while Instant::now().duration_since(start) < Duration::from_millis(TEST_TIME_MS as u64)
            {
                info_limit!(
                    MAX_LOGS_PER_PERIOD,
                    Duration::from_millis(TEST_PERIOD_MS as u64),
                    "Logging on repeat"
                );
            }
        })
    }

    #[test]
    fn sync_spamming_does_not_work_sync() {
        spamming_does_not_work(|| {
            let start = Box::new(Instant::now());
            let start = Box::leak(start);
            let handles: Vec<_> = (0..1)
                .map(|_| {
                    std::thread::spawn(|| {
                        while Instant::now().duration_since(*start)
                            < Duration::from_millis(TEST_TIME_MS as u64)
                        {
                            info_limit_global!(
                                MAX_LOGS_PER_PERIOD,
                                Duration::from_millis(TEST_PERIOD_MS as u64),
                                "Logging on repeat"
                            );
                        }
                    })
                })
                .collect();
            for handle in handles {
                handle.join().unwrap();
            }
        })
    }

    #[test]
    fn all_synchronised_variants_compile() {
        error_limit_global!(1, Duration::from_millis(1), "");
        warn_limit_global!(1, Duration::from_millis(1), "");
        info_limit_global!(1, Duration::from_millis(1), "");
        debug_limit_global!(1, Duration::from_millis(1), "");
        trace_limit_global!(1, Duration::from_millis(1), "");
    }

    #[test]
    fn all_thread_variants_compile() {
        error_limit!(1, Duration::from_millis(1), "");
        warn_limit!(1, Duration::from_millis(1), "");
        info_limit!(1, Duration::from_millis(1), "");
        debug_limit!(1, Duration::from_millis(1), "");
        trace_limit!(1, Duration::from_millis(1), "");
    }
}
