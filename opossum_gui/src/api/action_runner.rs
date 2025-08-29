use dioxus::prelude::*;

use crate::{LOADING, OPOSSUM_UI_LOGS};
use std::future::Future;

/// A universal async action runner for API calls that:
/// - Automatically manages a loading state in the UI.
/// - Executes optional callbacks on success or error.
/// - Centralizes error logging (if no error callback is provided).
///
/// # Type Parameters
/// * `F`: The type of the Future representing the API call.
/// * `T`: The type returned by a successful API call.
/// * `S`: The type of the success callback.
///
/// # Arguments
/// * `fut` - The future (async operation) representing the API call.
/// * `on_success` - Optional callback executed on success (`Ok`), receives the successful value.
pub async fn run_action<F, T, S>(fut: F, on_success: Option<S>)
where
    F: Future<Output = Result<T, String>> + 'static, // No `Send` required
    S: FnOnce(T) + 'static,
{
    *LOADING.write() = true;
    match fut.await {
        Ok(value) => {
            if let Some(callback) = on_success {
                callback(value);
            }
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
    *LOADING.write() = false;
}
