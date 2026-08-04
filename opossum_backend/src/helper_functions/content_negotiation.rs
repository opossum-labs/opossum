use std::pin::Pin;

use actix_web::{FromRequest, HttpRequest, HttpResponse, dev::Payload};
use serde::{Serialize, de::DeserializeOwned};

use crate::error::BackEndErrorResponse;

/// Custom extractor to handle Rusty Object Notation (RON) payloads
pub struct Ron<T>(pub T);

impl<T> Ron<T> {
    /// Deconstruct to get the inner value
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> FromRequest for Ron<T>
where
    T: DeserializeOwned + 'static,
{
    // Use your custom error response type directly
    type Error = BackEndErrorResponse;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        // Reuse Actix's built-in String extractor to read the request body
        let string_fut = String::from_request(req, payload);

        Box::pin(async move {
            // 1. Extract the raw string payload and map potential Actix errors
            let body_str = string_fut.await.map_err(|err| {
                BackEndErrorResponse::new(
                    400,
                    "Payload Error",
                    &format!("Failed to read request body: {err}"),
                )
            })?;

            // 2. Deserialize the RON string into the target type T
            let data = ron::de::from_str(&body_str).map_err(|err| {
                BackEndErrorResponse::new(
                    400,
                    "Parse Error",
                    &format!("Failed to deserialize payload: {err}"),
                )
            })?;

            Ok(Self(data))
        })
    }
}

/// Serializes `value` as the response body, honoring content negotiation between RON and JSON.
///
/// If `req`'s `Accept` header contains `application/ron`, `value` is serialized to RON (using
/// pretty formatting), since RON can represent `NaN`/`Inf` values that JSON cannot. Otherwise the
/// response falls back to JSON.
///
/// # Errors
///
/// Returns an error if RON serialization fails.
pub fn ron_or_json_response<T: Serialize>(
    req: &HttpRequest,
    value: &T,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let wants_ron = req
        .headers()
        .get(actix_web::http::header::ACCEPT)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|s| s.contains("application/ron"));

    if wants_ron {
        let body = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::new().new_line("\n"))
            .map_err(|e| BackEndErrorResponse::new(500, "Serialization Error", &e.to_string()))?;
        Ok(HttpResponse::Ok()
            .content_type("application/ron")
            .body(body))
    } else {
        Ok(HttpResponse::Ok().json(value))
    }
}
