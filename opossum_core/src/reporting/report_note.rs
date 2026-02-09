use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ReportLevel {
    Info,
    Warning,
    Error,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ReportNote {
    pub level: ReportLevel,
    pub message: String,
}

impl ReportNote {
    #[must_use]
    pub fn new(level: ReportLevel, message: &str) -> Self {
        Self {
            level,
            message: message.to_owned(),
        }
    }
}
