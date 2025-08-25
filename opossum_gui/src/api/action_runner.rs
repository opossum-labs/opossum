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
pub fn run_action<F, T, S>(fut: F, on_success: Option<S>)
where
    F: Future<Output = Result<T, String>> + 'static, // No `Send` required
    S: FnOnce(T) + 'static,
{
    spawn(async move {
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
    });
}

#[allow(dead_code)]
/// Extended version of [`run_action`] that allows providing a **custom error callback**.
///
/// # Arguments
/// * `fut`        - Async API call future.
/// * `on_success` - Optional closure executed on success.
/// * `on_error`   - closure executed on error.
pub fn run_action_with_success_check<F, T, S>(
    fut: F,
    on_success: Option<S>,
    mut action_successful: Signal<bool>,
) where
    F: Future<Output = Result<T, String>> + 'static, // No `Send` required
    S: FnOnce(T) + 'static,
{
    spawn(async move {
        *LOADING.write() = true;
        match fut.await {
            Ok(value) => {
                if let Some(callback) = on_success {
                    callback(value);
                }
                action_successful.set(true);
            }
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                action_successful.set(false);
            }
        }
        *LOADING.write() = false;
    });
}
