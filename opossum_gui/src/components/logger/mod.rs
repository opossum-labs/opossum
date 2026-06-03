pub mod logger_component;
use chrono::{self, Timelike};
use dioxus::prelude::*;

use crate::OPOSSUM_UI_LOGS;

#[derive(Clone)]
pub struct Logs {
    logs: Signal<Vec<String>>,
}

impl Default for Logs {
    fn default() -> Self {
        Self::new()
    }
}

impl Logs {
    #[must_use]
    pub fn new() -> Self {
        Self {
            logs: Signal::new(Vec::<String>::new()),
        }
    }
    #[must_use]
    pub const fn logs(&self) -> Signal<Vec<String>> {
        self.logs
    }
    pub fn add_log(&self, log_msg: &str) {
        let dt = chrono::offset::Local::now();
        self.logs().write().push(format!(
            "{:0>2}:{:0>2}:{:0>2} [log]:\t{}",
            dt.hour(),
            dt.minute(),
            dt.second(),
            log_msg
        ));
    }
}

pub trait LogResultExt {
    fn log_err_with_context(self, context: &str);
    #[allow(dead_code)]
    fn log_err(self);
}

impl<T, E> LogResultExt for Result<T, E>
where
    E: std::fmt::Display,
{
    fn log_err_with_context(self, context: &str) {
        if let Err(e) = &self {
            OPOSSUM_UI_LOGS
                .write()
                .add_log(&format!("Error in {context}: {e}"));
        }
    }

    fn log_err(self) {
        if let Err(e) = self {
            OPOSSUM_UI_LOGS.write().add_log(&format!("Error: {e}"));
        }
    }
}
