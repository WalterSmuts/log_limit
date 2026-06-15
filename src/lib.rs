#![doc = include_str!("../README.md")]

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

#[cfg(test)]
mod testing_logger;

/// A throttling event handed to the reporter installed via [`set_reporter`].
/// `context` identifies the throttled log line (source location, plus any
/// caller-supplied label).
pub enum ThrottleNotice<'a> {
    /// The threshold was just reached; further logs from this site are dropped
    /// for up to `within`.
    Throttling {
        context: core::fmt::Arguments<'a>,
        within: Duration,
    },
    /// Logging from this site resumed after `dropped` messages were suppressed
    /// over `elapsed`.
    Resumed {
        context: core::fmt::Arguments<'a>,
        dropped: usize,
        elapsed: Duration,
    },
}

type Reporter = dyn Fn(&ThrottleNotice) + Send + Sync;
static REPORTER: OnceLock<Box<Reporter>> = OnceLock::new();

/// Install the reporter used to emit throttle/recovery notices. May be set
/// once; later calls are ignored. The reporter runs only on throttle events
/// (never on the per-log hot path) and should emit via the `log` crate
/// directly to avoid recursing through a rate-limited macro.
pub fn set_reporter<F>(reporter: F)
where
    F: Fn(&ThrottleNotice) + Send + Sync + 'static,
{
    let _ = REPORTER.set(Box::new(reporter));
}

#[cfg(feature = "warning-messages")]
fn report(notice: &ThrottleNotice) {
    match REPORTER.get() {
        Some(reporter) => reporter(notice),
        None => default_report(notice),
    }
}

// Preserves the original warning wording. The `context` is available to a
// custom reporter via `set_reporter`, but the default text ignores it.
#[cfg(feature = "warning-messages")]
fn default_report(notice: &ThrottleNotice) {
    match notice {
        ThrottleNotice::Throttling { within, .. } => {
            log::warn!(
                "Hit logging threshold! Starting to ignore the previous log for {:.2?}",
                within
            );
        }
        ThrottleNotice::Resumed {
            dropped, elapsed, ..
        } => {
            log::warn!(
                "Ignored {dropped} logs since {:.2?} ago. Starting to log again...",
                elapsed
            );
        }
    }
}

#[doc(hidden)]
pub struct RateLimiter {
    count: usize,
    timestamp: Instant,
    logged_timeout: bool,
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
            logged_timeout: false,
        }
    }

    #[cfg_attr(not(feature = "warning-messages"), allow(unused_variables))]
    pub fn log_maybe(
        &mut self,
        period: Duration,
        max_per_time: usize,
        context: core::fmt::Arguments,
        log: impl Fn(),
    ) {
        let now = Instant::now();

        #[cfg(feature = "warning-messages")]
        let calculated_duration = now.duration_since(self.timestamp);
        if self.count < max_per_time {
            log();
            self.count += 1;

            #[cfg(feature = "warning-messages")]
            if self.count == max_per_time && period >= calculated_duration {
                report(&ThrottleNotice::Throttling {
                    context,
                    within: period - calculated_duration,
                });
                self.logged_timeout = true;
            }
        } else {
            let calculated_duration = now.duration_since(self.timestamp);
            if calculated_duration > period {
                #[cfg(feature = "warning-messages")]
                let filtered_log_count = self.count - max_per_time;
                #[cfg(feature = "warning-messages")]
                if self.logged_timeout {
                    report(&ThrottleNotice::Resumed {
                        context,
                        dropped: filtered_log_count,
                        elapsed: calculated_duration,
                    });
                }
                self.logged_timeout = false;
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

    #[cfg_attr(not(feature = "warning-messages"), allow(unused_variables))]
    pub fn log_maybe(
        &self,
        period: Duration,
        max_per_time: usize,
        context: core::fmt::Arguments,
        log: impl Fn(),
    ) {
        let count = self.count.fetch_add(1, Ordering::Relaxed) + 1;
        if count <= max_per_time {
            log();
            #[cfg(feature = "warning-messages")]
            if count == max_per_time {
                report(&ThrottleNotice::Throttling {
                    context,
                    within: period,
                });
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
                report(&ThrottleNotice::Resumed {
                    context,
                    dropped: filtered_log_count,
                    elapsed: calculated_duration,
                });
                log();
                *timestamp = now;
            }
        }
    }
}

// Helper macro to reduce duplication in global limit macros
#[doc(hidden)]
#[macro_export]
macro_rules! global_limit_impl {
    ($level:expr, context: $context:expr, $max_per_time:expr, $period:expr, target: $target:expr, $($arg:tt)+) => {{
        use $crate::SynchronisedRateLimiter;
        use std::sync::LazyLock;
        if log::log_enabled!($level) {
            static RATE_LIMITER: LazyLock<SynchronisedRateLimiter> = SynchronisedRateLimiter::new();
            RATE_LIMITER.log_maybe($period, $max_per_time, $context, || log::log!(target: $target, $level, $($arg)+));
        }
    }};
    ($level:expr, context: $context:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::SynchronisedRateLimiter;
        use std::sync::LazyLock;
        if log::log_enabled!($level) {
            static RATE_LIMITER: LazyLock<SynchronisedRateLimiter> = SynchronisedRateLimiter::new();
            RATE_LIMITER.log_maybe($period, $max_per_time, $context, || log::log!($level, $($arg)+));
        }
    }};
    ($level:expr, $max_per_time:expr, $period:expr, target: $target:expr, $($arg:tt)+) => {
        $crate::global_limit_impl!($level, context: format_args!("{}:{}", file!(), line!()), $max_per_time, $period, target: $target, $($arg)+)
    };
    ($level:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::global_limit_impl!($level, context: format_args!("{}:{}", file!(), line!()), $max_per_time, $period, $($arg)+)
    };
}

#[macro_export]
macro_rules! error_limit_global {
    (target: $target:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::global_limit_impl!(log::Level::Error, $max_per_time, $period, target: $target, $($arg)+)
    };
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::global_limit_impl!(log::Level::Error, $max_per_time, $period, $($arg)+)
    };
}

#[macro_export]
macro_rules! warn_limit_global {
    (target: $target:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::global_limit_impl!(log::Level::Warn, $max_per_time, $period, target: $target, $($arg)+)
    };
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::global_limit_impl!(log::Level::Warn, $max_per_time, $period, $($arg)+)
    };
}

#[macro_export]
macro_rules! info_limit_global {
    (target: $target:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::global_limit_impl!(log::Level::Info, $max_per_time, $period, target: $target, $($arg)+)
    };
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::global_limit_impl!(log::Level::Info, $max_per_time, $period, $($arg)+)
    };
}

#[macro_export]
macro_rules! debug_limit_global {
    (target: $target:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::global_limit_impl!(log::Level::Debug, $max_per_time, $period, target: $target, $($arg)+)
    };
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::global_limit_impl!(log::Level::Debug, $max_per_time, $period, $($arg)+)
    };
}

#[macro_export]
macro_rules! trace_limit_global {
    (target: $target:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::global_limit_impl!(log::Level::Trace, $max_per_time, $period, target: $target, $($arg)+)
    };
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::global_limit_impl!(log::Level::Trace, $max_per_time, $period, $($arg)+)
    };
}

// Helper macro to reduce duplication in thread-local limit macros
#[doc(hidden)]
#[macro_export]
macro_rules! limit_impl {
    ($level:expr, context: $context:expr, $max_per_time:expr, $period:expr, target: $target:expr, $($arg:tt)+) => {{
        use $crate::RateLimiter;
        use std::cell::RefCell;
        use std::thread_local;

        if log::log_enabled!($level) {
            thread_local! {
                static RATE_LIMITER: RefCell<RateLimiter> = RefCell::new(RateLimiter::new());
            }

            RATE_LIMITER.with(|rate_limiter| {
                rate_limiter
                    .borrow_mut()
                    .log_maybe($period, $max_per_time, $context, || log::log!(target: $target, $level, $($arg)+))
            });
        }
    }};
    ($level:expr, context: $context:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use $crate::RateLimiter;
        use std::cell::RefCell;
        use std::thread_local;

        if log::log_enabled!($level) {
            thread_local! {
                static RATE_LIMITER: RefCell<RateLimiter> = RefCell::new(RateLimiter::new());
            }

            RATE_LIMITER.with(|rate_limiter| {
                rate_limiter
                    .borrow_mut()
                    .log_maybe($period, $max_per_time, $context, || log::log!($level, $($arg)+))
            });
        }
    }};
    ($level:expr, $max_per_time:expr, $period:expr, target: $target:expr, $($arg:tt)+) => {
        $crate::limit_impl!($level, context: format_args!("{}:{}", file!(), line!()), $max_per_time, $period, target: $target, $($arg)+)
    };
    ($level:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::limit_impl!($level, context: format_args!("{}:{}", file!(), line!()), $max_per_time, $period, $($arg)+)
    };
}

#[macro_export]
macro_rules! error_limit {
    (target: $target:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::limit_impl!(log::Level::Error, $max_per_time, $period, target: $target, $($arg)+)
    };
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::limit_impl!(log::Level::Error, $max_per_time, $period, $($arg)+)
    };
}

#[macro_export]
macro_rules! warn_limit {
    (target: $target:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::limit_impl!(log::Level::Warn, $max_per_time, $period, target: $target, $($arg)+)
    };
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::limit_impl!(log::Level::Warn, $max_per_time, $period, $($arg)+)
    };
}

#[macro_export]
macro_rules! info_limit {
    (target: $target:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::limit_impl!(log::Level::Info, $max_per_time, $period, target: $target, $($arg)+)
    };
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::limit_impl!(log::Level::Info, $max_per_time, $period, $($arg)+)
    };
}

#[macro_export]
macro_rules! debug_limit {
    (target: $target:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::limit_impl!(log::Level::Debug, $max_per_time, $period, target: $target, $($arg)+)
    };
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::limit_impl!(log::Level::Debug, $max_per_time, $period, $($arg)+)
    };
}

#[macro_export]
macro_rules! trace_limit {
    (target: $target:expr, $max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::limit_impl!(log::Level::Trace, $max_per_time, $period, target: $target, $($arg)+)
    };
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {
        $crate::limit_impl!(log::Level::Trace, $max_per_time, $period, $($arg)+)
    };
}

#[cfg(test)]
mod tests {
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
            // 22: Omit
            // 33: Omit
            // 44: Omit
            // 55: Log (and warn: missed 3)
            // 66: Log (and warn of omission)
            // 77: Omit
            // 88: Omit
            // 99: Omit
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

            // Ensure we don't overstep the limit on average
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
    fn sync_spamming_does_not_work() {
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

    #[test]
    fn target_parameter_works() {
        error_limit!(target: "custom_target", 1, Duration::from_millis(1), "");
        warn_limit!(target: "custom_target", 1, Duration::from_millis(1), "");
        info_limit!(target: "custom_target", 1, Duration::from_millis(1), "");
        debug_limit!(target: "custom_target", 1, Duration::from_millis(1), "");
        trace_limit!(target: "custom_target", 1, Duration::from_millis(1), "");

        error_limit_global!(target: "custom_target", 1, Duration::from_millis(1), "");
        warn_limit_global!(target: "custom_target", 1, Duration::from_millis(1), "");
        info_limit_global!(target: "custom_target", 1, Duration::from_millis(1), "");
        debug_limit_global!(target: "custom_target", 1, Duration::from_millis(1), "");
        trace_limit_global!(target: "custom_target", 1, Duration::from_millis(1), "");
    }

    #[test]
    fn max_per_time_longer_than_period_overflow() {
        crate::testing_logger::setup();
        fn log_function() {
            info_limit!(3, Duration::from_millis(10), "");
        }

        // Trigger threshold
        log_function();
        log_function();
        log_function();
        log_function();

        // Reset after period was reached and record timestamp
        thread::sleep(Duration::from_millis(11));
        log_function();

        // Get to just one log before hitting the threshold

        // Ensure time delta is longer than period
        thread::sleep(Duration::from_millis(11));
        log_function();
        log_function();
    }

    #[test]
    fn thread_local_suppressed_levels_are_ignored() {
        crate::testing_logger::setup();

        // Restrict logging to Warn and above
        log::set_max_level(log::LevelFilter::Warn);

        for _ in 0..10 {
            debug_limit!(
                2,
                Duration::from_secs(1),
                "This debug message is suppressed"
            );
        }

        crate::testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 0,);
        });
    }

    #[test]
    fn global_suppressed_levels_are_ignored() {
        crate::testing_logger::setup();

        // Restrict logging to Warn and above
        log::set_max_level(log::LevelFilter::Warn);

        for _ in 0..10 {
            debug_limit_global!(
                2,
                Duration::from_secs(1),
                "This global debug message is suppressed"
            );
        }

        crate::testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 0,);
        });
    }
}
