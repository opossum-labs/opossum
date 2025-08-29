use crate::OPOSSUM_UI_LOGS;

/// A universal response handler for async calls that
/// - Executes optional callbacks on success or error.
/// - Centralizes error logging (if no error callback is provided).
///
/// # Type Parameters
/// * `T`: The type returned by a successful API call.
/// * `S`: The type of the success callback.
///
/// # Arguments
/// * `res` - The response of the server
/// * `on_success` - Optional callback executed on success (`Ok`), receives the successful value.
pub fn eval_action_run<T, S>(res: Result<T, String>, on_success: Option<S>)
where
    S: FnOnce(T) + 'static,
{
    match res {
        Ok(value) => {
            if let Some(callback) = on_success {
                callback(value);
            }
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
}
