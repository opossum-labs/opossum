use crate::HTTP_API_CLIENT;
use futures_util::StreamExt;
use opossum_core::opm_document::OptimizationConfig;
use opossum_optimization::{ModelOptimizationSchema, OptimizationProgressEvent};

/// Fetches the optimization schema from the currently loaded model.
///
/// This includes all controllable variables (lens properties, distances),
/// observable metrics from detectors, and elements supporting geometric constraints.
///
/// # Errors
///
/// Returns an error if the HTTP request fails or the response cannot be deserialized.
pub async fn get_optimization_schema() -> Result<ModelOptimizationSchema, String> {
    HTTP_API_CLIENT()
        .get::<ModelOptimizationSchema>("/api/optimization/schema")
        .await
}

/// Sends an optimization configuration to the backend to start a solver job.
///
/// The backend spawns a worker thread, computes initial costs, and streams
/// progress events via SSE.
///
/// # Arguments
///
/// * `config` - Complete optimization setup (variables, couplings, operands, algorithm).
///
/// # Errors
///
/// Returns an error if another optimization job is already running (HTTP 409)
/// or if serialization/parsing fails (HTTP 400).
pub async fn post_start_optimization(config: OptimizationConfig) -> Result<String, String> {
    HTTP_API_CLIENT()
        .post_ron::<OptimizationConfig, String>("/api/optimization/start", config)
        .await
}

/// Saves an optimization configuration to the loaded document in the backend.
pub async fn post_save_optimization_config(config: OptimizationConfig) -> Result<String, String> {
    HTTP_API_CLIENT()
        .post_ron::<OptimizationConfig, String>("/api/optimization/config", config)
        .await
}

/// Requests the backend to abort an actively running optimization job.
///
/// When aborted, the optimizer safely restores or retains the best parameter
/// state found so far and records an undo step.
///
/// # Errors
///
/// Returns an error if the request fails to reach the backend.
pub async fn post_abort_optimization() -> Result<String, String> {
    HTTP_API_CLIENT()
        .post_no_body::<String>("/api/optimization/abort")
        .await
}

/// Connects to the Server-Sent Events (SSE) stream at `/api/optimization/events`
/// and invokes the provided callback for each received event.
///
/// An optional `on_connected` oneshot sender can be provided, which is fired as soon
/// as the HTTP connection to the SSE endpoint is successfully established.
///
/// # Arguments
///
/// * `on_connected` - Optional one-shot notification when the SSE connection is established.
/// * `on_event` - Closure invoked whenever an `OptimizationProgressEvent` arrives.
///
/// # Errors
///
/// Returns an error if the HTTP connection fails or the SSE stream cannot be established.
pub async fn subscribe_optimization_events<F>(
    on_connected: Option<tokio::sync::oneshot::Sender<()>>,
    mut on_event: F,
) -> Result<(), String>
where
    F: FnMut(OptimizationProgressEvent) + Send + 'static,
{
    let url = HTTP_API_CLIENT().url("/api/optimization/events");
    let response = HTTP_API_CLIENT()
        .client()
        .get(&url)
        .header("Accept", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .send()
        .await
        .map_err(|e| format!("Failed to connect to SSE stream at '{url}': {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "SSE subscription failed with HTTP status {}",
            response.status()
        ));
    }

    // Signal that the SSE stream connection is now live
    if let Some(tx) = on_connected {
        let _ = tx.send(());
    }

    let mut stream = response.bytes_stream();
    let mut line_buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("Error receiving SSE chunk: {e}"))?;
        let text = String::from_utf8_lossy(&chunk);
        line_buffer.push_str(&text);

        // Process full lines terminated by newline characters
        while let Some(newline_pos) = line_buffer.find('\n') {
            let line = line_buffer[..newline_pos].trim().to_string();
            line_buffer.drain(..=newline_pos);

            // Standard SSE format prefix: "data: <json>"
            if let Some(json_payload) = line.strip_prefix("data:") {
                let trimmed_json = json_payload.trim();
                if trimmed_json.is_empty() {
                    continue;
                }

                match serde_json::from_str::<OptimizationProgressEvent>(trimmed_json) {
                    Ok(event) => {
                        let should_terminate = matches!(
                            event,
                            OptimizationProgressEvent::Finished { .. }
                                | OptimizationProgressEvent::Aborted { .. }
                                | OptimizationProgressEvent::Failed { .. }
                        );

                        on_event(event);

                        // If the job completed or failed, close the receiver loop
                        if should_terminate {
                            return Ok(());
                        }
                    }
                    Err(err) => {
                        log::warn!(
                            "Failed to parse SSE event: {err}. Raw payload: '{trimmed_json}'"
                        );
                    }
                }
            }
        }
    }

    // If stream ended without a terminal event, notify client to prevent hanging state
    on_event(OptimizationProgressEvent::Failed {
        error: "SSE event stream closed unexpectedly by server.".to_string(),
    });

    Ok(())
}
