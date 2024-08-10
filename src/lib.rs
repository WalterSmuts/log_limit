#[macro_export]
macro_rules! info_limit {
    ($max_per_time:expr, $period:expr, $($arg:tt)+) => {{
        use std::sync::atomic::AtomicUsize;
        use std::sync::atomic::Ordering;
        use std::sync::Mutex;
        use std::time::Duration;
        use std::time::Instant;
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        static TIMESTAMP: Mutex<Option<Instant>> = Mutex::new(None);
        {
            // TODO: Find a better way to initialize this
            let mut maybe_timestamp = TIMESTAMP.lock().unwrap();
            if maybe_timestamp.is_none() {
                *maybe_timestamp = Some(Instant::now());
            }
        }

        if COUNT.fetch_add(1, Ordering::Relaxed) < $max_per_time {
            log::info!($($arg)+);
        } else {
            let now = Instant::now();
            // Safe to unwrap here because we will never panic *touch wood*
            let mut maybe_timestamp = TIMESTAMP.lock().unwrap();
            // Safe to unwap here because we always populate with Some above if there is a none and
            // we never initialize with a none.
            if now.duration_since(maybe_timestamp.unwrap()) > $period {
                let filtered_log_count = COUNT.swap(0, Ordering::Relaxed);
                log::info!("Ignored {filtered_log_count} since {maybe_timestamp:?}");
                log::info!($($arg)+);
                *maybe_timestamp = Some(now);
            }
        }
    };
    }
}

#[cfg(test)]
mod tests {
    use simple_logger::SimpleLogger;

    use super::*;

    #[test]
    fn it_works() {
        SimpleLogger::new().init().unwrap();
        let a = 1;
        info_limit!(10, Duration::from_secs(60), "Logging {}", 10);
        info_limit!(1, Duration::from_secs(60), "Logging");
        for _ in 0..10 {
            info_limit!(1, Duration::from_secs(60), "Logging on repeat{a}");
        }
        panic!()
    }
}
