use log::{Level, LevelFilter, Log, Metadata, Record};
use std::{cell::RefCell, sync::Once};
use tokio::sync::mpsc;

// A thread-local variable to hold the sender while logging to an HTTP stream.
thread_local! {
    pub (crate) static SENDER: RefCell<Option<mpsc::Sender<String>>> = const {RefCell::new(None)};
}

// Our custom logger struct.
pub struct SseLogger;

// Implementation of the `log::Log` trait.
impl Log for SseLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        // Log all messages of level INFO or higher.
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            let log_type = record.level().as_str();
            let log_message = format!("{}##{}", log_type, record.args());

            // Try to send the log message through the thread-local sender.
            let sent = SENDER.with(|cell| {
                if let Some(sender) = cell.borrow().as_ref() {
                    // Use `blocking_send` because we are in a sync context (the logger).
                    // This is safe because the whole operation runs in a blocking thread.
                    return sender.blocking_send(log_message.clone()).is_ok();
                }
                false
            });

            // If it wasn't sent (no sender configured for this thread),
            // just print it to the console as a fallback.
            if !sent {
                println!("{log_message}");
            }
        }
    }
    fn flush(&self) {}
}

// Global static instance of our logger.
static LOGGER: SseLogger = SseLogger;
static INIT: Once = Once::new();

/// Initialize the global logger.
///
/// # Panics
///
/// Panics if the logger could not be initialized.
pub fn init_logger() {
    INIT.call_once(|| {
        log::set_logger(&LOGGER)
            .map(|()| log::set_max_level(LevelFilter::Info))
            .expect("Failed to set logger");
    });
}
