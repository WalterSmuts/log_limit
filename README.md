# log_limit

A rate limiting logging crate. Simply wraps the [log] crate with logic to
ignore writing logs if that specific log-line is called too often. This is
controlled by a `threshold` and a `period`. If the theshold is reached the log
is ignored for the rest of the period. Warnings are \[optionally\] logged to
inform the user that the log is being ignored when the threshold is hit. When
the next period starts, another warning is logged, reporting the number of logs
that were ignored.

### Properties
* The rate-limiting only applies for the single lexical (in-code) log line
* On average the log emission rate won't exceed the rate specified by
  `threshold` / `period`
* The log emission is bursty to maintain sequential order of logs, i.e. either
  emitting all or nothing
* Any arbitrary `period` will contain <= 2x the threshold amount of logs
  (because two bursts can "just" fall in one arbitrary `period`)

### Variants
1. *Normal* - rate limit applies to a single thread.
    1. `[error|warn|info|debug|trace]_limit!`
    1. Uses [thread_local] storage
1. *Global* - rate limit applies to the entire process.
    1. `[error|warn|info|debug|trace]_limit_global!`
    1. Uses atomics to synchronise
    1. Takes a global lock (only when over the threshold)

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
```

#### Produces:
```txt
2024-08-24T10:49:29.197Z DEBUG [log_limit_user] Loop number: 0
2024-08-24T10:49:29.197Z ERROR [log_limit_user] Rate limit log for 0
2024-08-24T10:49:29.198Z DEBUG [log_limit_user] Loop number: 1
2024-08-24T10:49:29.198Z ERROR [log_limit_user] Rate limit log for 1
2024-08-24T10:49:29.199Z DEBUG [log_limit_user] Loop number: 2
2024-08-24T10:49:29.199Z ERROR [log_limit_user] Rate limit log for 2
2024-08-24T10:49:29.199Z WARN  [log_limit] Hit logging threashold! Starting to ignore the previous log for 2.218167ms
2024-08-24T10:49:29.200Z DEBUG [log_limit_user] Loop number: 3
2024-08-24T10:49:29.201Z DEBUG [log_limit_user] Loop number: 4
2024-08-24T10:49:29.203Z DEBUG [log_limit_user] Loop number: 5
2024-08-24T10:49:29.203Z WARN  [log_limit] Ignored 2 logs since 5.515993ms ago. Starting to log again...
2024-08-24T10:49:29.203Z ERROR [log_limit_user] Rate limit log for 5
2024-08-24T10:49:29.204Z DEBUG [log_limit_user] Loop number: 6
2024-08-24T10:49:29.204Z ERROR [log_limit_user] Rate limit log for 6
2024-08-24T10:49:29.205Z DEBUG [log_limit_user] Loop number: 7
2024-08-24T10:49:29.205Z ERROR [log_limit_user] Rate limit log for 7
2024-08-24T10:49:29.205Z WARN  [log_limit] Hit logging threashold! Starting to ignore the previous log for 2.181079ms
2024-08-24T10:49:29.206Z DEBUG [log_limit_user] Loop number: 8
2024-08-24T10:49:29.207Z DEBUG [log_limit_user] Loop number: 9
```

### TODO:
* Do some benchmarking and optimization
* Address all in-code TODO's.
* Figure out why my macro API looks different to the logging one? What is the target?
* Figure out the right API

[log]: https://docs.rs/log/latest/log/
[thread_local]: https://doc.rust-lang.org/std/macro.thread_local.htmlhttps://doc.rust-lang.org/std/macro.thread_local.html
