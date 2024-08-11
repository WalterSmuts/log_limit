# log_limit

A rate limiting logging crate.

### Example:
```rust
use std::thread;
use std::time::Duration;

use log_limit::info_limit;
use simple_logger::SimpleLogger;

SimpleLogger::new().init().unwrap();
for i in 0..10 {
    log::debug!("Loop number: {i}");
    info_limit!(3, Duration::from_millis(5), "Rate limit log for {i}");
    thread::sleep(Duration::from_millis(1));
}

// Produces:
//
// 2024-08-10T15:45:41.278Z DEBUG [log_limit_user] Loop number: 0
// 2024-08-10T15:45:41.278Z INFO  [log_limit_user] Rate limit log for 0
// 2024-08-10T15:45:41.279Z DEBUG [log_limit_user] Loop number: 1
// 2024-08-10T15:45:41.279Z INFO  [log_limit_user] Rate limit log for 1
// 2024-08-10T15:45:41.280Z DEBUG [log_limit_user] Loop number: 2
// 2024-08-10T15:45:41.280Z INFO  [log_limit_user] Rate limit log for 2
// 2024-08-10T15:45:41.280Z WARN  [log_limit] Starting to ignore the previous log for less than 5ms
// 2024-08-10T15:45:41.281Z DEBUG [log_limit_user] Loop number: 3
// 2024-08-10T15:45:41.282Z DEBUG [log_limit_user] Loop number: 4
// 2024-08-10T15:45:41.283Z DEBUG [log_limit_user] Loop number: 5
// 2024-08-10T15:45:41.283Z WARN  [log_limit] Ignored 3 logs since more than 5ms ago. Starting again...
// 2024-08-10T15:45:41.283Z INFO  [log_limit_user] Rate limit log for 5
// 2024-08-10T15:45:41.285Z DEBUG [log_limit_user] Loop number: 6
// 2024-08-10T15:45:41.285Z INFO  [log_limit_user] Rate limit log for 6
// 2024-08-10T15:45:41.286Z DEBUG [log_limit_user] Loop number: 7
// 2024-08-10T15:45:41.286Z INFO  [log_limit_user] Rate limit log for 7
// 2024-08-10T15:45:41.286Z WARN  [log_limit] Starting to ignore the previous log for less than 5ms
// 2024-08-10T15:45:41.287Z DEBUG [log_limit_user] Loop number: 8
// 2024-08-10T15:45:41.288Z DEBUG [log_limit_user] Loop number: 9
```

### TODO:
* Implement a thread-local variant (can you even make it sound?)
* Do some benchmarking and optimization
* Address all in-code TODO's.
* Figure out if there are more use-cases and configuration required
* Figure out why my macro API looks different to the logging one? What is the target?
