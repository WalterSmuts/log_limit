use log::Level;
use log::LevelFilter;
use log::Log;
use log::Metadata;
use log::Record;
use std::sync::Mutex;
use std::sync::Once;

/// A captured call to the logging system. A `Vec` of these is passed
/// to the closure supplied to the `validate()` function.
pub struct CapturedLog {
    #[cfg(feature = "warning-messages")]
    /// The formatted log message.
    pub body: String,
    /// The level.
    pub level: Level,
}

static LOG_RECORDS: Mutex<Vec<CapturedLog>> = Mutex::new(Vec::new());

struct TestingLogger {}

impl Log for TestingLogger {
    #[allow(unused_variables)]
    fn enabled(&self, metadata: &Metadata) -> bool {
        true // capture all log levels
    }

    fn log(&self, record: &Record) {
        let mut records = LOG_RECORDS.lock().unwrap();
        let captured_record = CapturedLog {
            #[cfg(feature = "warning-messages")]
            body: format!("{}", record.args()),
            level: record.level(),
        };
        records.push(captured_record);
    }

    fn flush(&self) {}
}

static FIRST_TEST: Once = Once::new();

static TEST_LOGGER: TestingLogger = TestingLogger {};

/// Prepare the `testing_logger` to capture log messages for a test.
///
/// Should be called from every test that calls `validate()`, before any calls to the logging system.
/// This function will install an internal `TestingLogger` as the logger if not already done so, and initialise
/// its thread local storage for a new test.
pub fn setup() {
    FIRST_TEST.call_once(|| {
        log::set_logger(&TEST_LOGGER)
            .map(|()| log::set_max_level(LevelFilter::Trace))
            .unwrap();
    });
    let mut records = LOG_RECORDS.lock().unwrap();
    records.truncate(0);
}

/// Used to validate any captured log events.
///
/// the `asserter` closure can check the number, body, target and level
/// of captured log events. As a side effect, the records are cleared.
pub fn validate<F>(asserter: F)
where
    F: Fn(&Vec<CapturedLog>),
{
    let mut records = LOG_RECORDS.lock().unwrap();
    asserter(&records);
    records.truncate(0);
}
