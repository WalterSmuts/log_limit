use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

pub fn info_limit(log_line: &str, max_per_time: usize, period: Duration) {
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    static TIMESTAMP: Mutex<Option<Instant>> = Mutex::new(None);

    if COUNT.fetch_add(1, Ordering::Relaxed) <= max_per_time {
        log::info!("{log_line}");
    } else {
        let now = Instant::now();
        let mut maybe_timestamp = TIMESTAMP.lock().unwrap();
        if now.duration_since(maybe_timestamp.unwrap()) > period {
            let filtered_log_count = COUNT.swap(0, Ordering::Relaxed);
            log::info!("Ignored {filtered_log_count} since {maybe_timestamp:?}");
            log::info!("{log_line}");
            *maybe_timestamp = Some(now);
        }
    }
}

#[cfg(test)]
mod tests {
    //use super::*;

    #[test]
    fn it_works() {}
}
