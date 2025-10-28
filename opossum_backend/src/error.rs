use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use opossum_core::{error::OpossumError, types::api_types::ErrorResponse};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BackEndErrorResponse(ErrorResponse);

impl BackEndErrorResponse {
    #[must_use]
    pub fn new(status: u16, category: &str, message: &str) -> Self {
        Self(ErrorResponse::new(status, category, message))
    }
    #[must_use]
    pub fn error_response(&self) -> ErrorResponse {
        self.0.clone()
    }
}
impl std::fmt::Display for BackEndErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.message())
    }
}

impl ResponseError for BackEndErrorResponse {
    fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.0.status()).unwrap()
    }
    fn error_response(&self) -> HttpResponse {
        let mut res = actix_web::HttpResponseBuilder::new(self.status_code());
        res.json(self.0.clone())
    }
}
impl From<OpossumError> for BackEndErrorResponse {
    fn from(error: OpossumError) -> Self {
        let (status, category) = match &error {
            OpossumError::OpmDocument(_) => (400, "OpmDocument".to_string()),
            OpossumError::OpticScenery(_) => (400, "OpticScenery".to_string()),
            OpossumError::OpticGroup(_) => (400, "OpticGroup".to_string()),
            OpossumError::OpticPort(_) => (400, "OpticPort".to_string()),
            OpossumError::Analysis(_) => (400, "Analysis".to_string()),
            OpossumError::Spectrum(_) => (400, "Spectrum".to_string()),
            OpossumError::Console(_) => (400, "Console".to_string()),
            OpossumError::Properties(_) => (400, "Properties".to_string()),
            OpossumError::Other(_) => (400, "Other".to_string()),
        };
        Self(ErrorResponse::new(status, &category, &error.to_string()))
    }
}
