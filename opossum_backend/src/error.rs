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
            OpossumError::Registry(_) => (400, "Registry"),
            OpossumError::Other(_) => (400, "Other"),
        };

        Self(ErrorResponse::new(status, category, &error.to_string()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{ResponseError, http::StatusCode};
    use opossum_core::error::OpossumError;

    #[test]
    fn test_error_constructors() {
        // Verify manual construction
        let err = BackEndErrorResponse::new(400, "CustomCategory", "Custom error message");
        assert_eq!(err.0.status, 400);
        assert_eq!(err.0.category, "CustomCategory");
        assert_eq!(err.0.message, "Custom error message");

        // Verify standard helpers
        let not_found = BackEndErrorResponse::not_found();
        assert_eq!(not_found.0.status, 404);
        assert_eq!(not_found.0.category, "NotFound");

        let analyzer_err = BackEndErrorResponse::analyzer_not_found();
        assert_eq!(analyzer_err.0.status, 404);
        assert_eq!(analyzer_err.0.category, "Opossum");

        let scenario_err = BackEndErrorResponse::pump_scenario_not_found();
        assert_eq!(scenario_err.0.status, 404);
        assert_eq!(scenario_err.0.category, "Opossum");
    }

    #[test]
    fn test_display_formatting() {
        let err = BackEndErrorResponse::new(404, "TestCat", "Detailed message");
        assert_eq!(format!("{err}"), "[404] TestCat: Detailed message");
    }

    #[test]
    fn test_response_error_status_code_and_response() {
        let err = BackEndErrorResponse::new(404, "NotFound", "Resource missing");
        assert_eq!(err.status_code(), StatusCode::NOT_FOUND);

        let resp = err.error_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // Fallback for invalid status numbers
        let invalid_err = BackEndErrorResponse::new(9999, "Invalid", "Invalid status number");
        assert_eq!(invalid_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_from_opossum_error_all_variants() {
        // Map each core error variant to its expected backend category
        let error_cases = vec![
            (OpossumError::OpmDocument("doc error".into()), "OpmDocument"),
            (
                OpossumError::OpticScenery("scenery error".into()),
                "OpticScenery",
            ),
            (OpossumError::OpticGroup("group error".into()), "OpticGroup"),
            (OpossumError::OpticPort("port error".into()), "OpticPort"),
            (OpossumError::Analysis("analysis error".into()), "Analysis"),
            (OpossumError::Spectrum("spectrum error".into()), "Spectrum"),
            (OpossumError::Console("console error".into()), "Console"),
            (OpossumError::Properties("props error".into()), "Properties"),
            (OpossumError::Registry("reg error".into()), "Registry"),
            (OpossumError::Other("other error".into()), "Other"),
        ];

        for (core_err, expected_category) in error_cases {
            let backend_err = BackEndErrorResponse::from(core_err);
            assert_eq!(backend_err.0.status, 400);
            assert_eq!(backend_err.0.category, expected_category);
        }
    }
}
