use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    /// HTTP status
    status: u16,
    /// Error category (normally corresponds to `OpossumError` enum)
    category: String,
    /// Description message of the error
    message: String,
}
impl ErrorResponse {
    #[must_use]
    pub fn new(status: u16, category: &str, message: &str) -> Self {
        Self {
            status,
            category: category.to_string(),
            message: message.to_string(),
        }
    }
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn category(&self) -> &str {
        &self.category
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn message(&self) -> &str {
        &self.message
    }
}
