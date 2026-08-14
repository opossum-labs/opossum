// In opossum_backend/src/error.rs
use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use opossum_core::{error::OpossumError, types::api_types::ErrorResponse};

/// Der Actix-kompatible Wrapper für unsere Fehler
#[derive(Debug)]
pub struct BackEndErrorResponse(pub ErrorResponse);

impl BackEndErrorResponse {
    pub fn new(status: u16, category: &str, message: &str) -> Self {
        Self(ErrorResponse::new(status, category, message))
    }
    pub fn not_found() -> Self {
        Self::new(404, "NotFound", "The requested resource was not found")
    }
    pub fn analyzer_not_found() -> Self {
        Self::new(404, "Opossum", "UUID not found in analyzers")
    }
    pub fn pump_scenario_not_found() -> Self {
        Self::new(404, "Opossum", "UUID not found in pump scenarios")
    }
}
// Display ist für Actix Pflicht
impl std::fmt::Display for BackEndErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {}: {}",
            self.0.status, self.0.category, self.0.message
        )
    }
}

impl ResponseError for BackEndErrorResponse {
    fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.0.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }

    fn error_response(&self) -> HttpResponse {
        // HIER IST DER TRICK: Wir serialisieren self.0 (das DTO) ans Frontend!
        let mut res = actix_web::HttpResponseBuilder::new(self.status_code());
        res.json(&self.0)
    }
}

// Übersetzung von Core-Fehlern in Backend-Fehler
impl From<OpossumError> for BackEndErrorResponse {
    fn from(error: OpossumError) -> Self {
        let (status, category) = match &error {
            OpossumError::OpmDocument(_) => (400, "OpmDocument"),
            OpossumError::OpticScenery(_) => (400, "OpticScenery"),
            OpossumError::OpticGroup(_) => (400, "OpticGroup"),
            OpossumError::OpticPort(_) => (400, "OpticPort"),
            OpossumError::Analysis(_) => (400, "Analysis"),
            OpossumError::Spectrum(_) => (400, "Spectrum"),
            OpossumError::Console(_) => (400, "Console"),
            OpossumError::Properties(_) => (400, "Properties"),
            OpossumError::Other(_) => (400, "Other"),
        };

        Self(ErrorResponse::new(status, category, &error.to_string()))
    }
}
