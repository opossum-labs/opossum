pub mod logger_component;
use chrono::{self, Timelike};
use dioxus::prelude::*;

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
    pub fn new() -> Self {
        Self {
            logs: Signal::new(Vec::<String>::new()),
        }
    }

    pub fn logs(&self) -> Signal<Vec<String>> {
        self.logs
    }

    pub fn add_log(&mut self, log_msg: String) {
        let dt = chrono::offset::Local::now();
        self.logs().write().push(format!(
            "{:0>2}:{:0>2}:{:0>2} [log]:\t{}",
            dt.hour(),
            dt.minute(),
            dt.second(),
            log_msg
        ))
    }
}
