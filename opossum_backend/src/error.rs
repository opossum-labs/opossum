use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use opossum::error::OpossumError;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Structure holding an error mesaage
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// HTTP status
    #[schema(example = "400")]
    status: u16,
    /// Error category (normally corresponds to `OpossumError` enum)
    #[schema(example = "OpticScenery")]
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
    pub fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND.as_u16(),
            category: "api not found".to_string(),
            message: "the OPOSSUM API endpoint was not found".to_string(),
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
impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<OpossumError> for ErrorResponse {
    fn from(error: OpossumError) -> Self {
        let (status, category) = match &error {
            OpossumError::OpmDocument(_) => (StatusCode::BAD_REQUEST, "OpmDocument".to_string()),
            OpossumError::OpticScenery(_) => (StatusCode::BAD_REQUEST, "OpticScenery".to_string()),
            OpossumError::OpticGroup(_) => (StatusCode::BAD_REQUEST, "OpticGroup".to_string()),
            OpossumError::OpticPort(_) => (StatusCode::BAD_REQUEST, "OpticPort".to_string()),
            OpossumError::Analysis(_) => (StatusCode::BAD_REQUEST, "Analysis".to_string()),
            OpossumError::Spectrum(_) => (StatusCode::BAD_REQUEST, "Spectrum".to_string()),
            OpossumError::Console(_) => (StatusCode::BAD_REQUEST, "Console".to_string()),
            OpossumError::Properties(_) => (StatusCode::BAD_REQUEST, "Properties".to_string()),
            OpossumError::Other(_) => (StatusCode::BAD_REQUEST, "Other".to_string()),
        };
        Self {
            status: status.as_u16(),
            category,
            message: error.to_string(),
        }
    }
}
impl ResponseError for ErrorResponse {
    fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.status).unwrap()
    }
    fn error_response(&self) -> HttpResponse {
        let mut res = actix_web::HttpResponseBuilder::new(self.status_code());
        res.json(self)
    }
}
