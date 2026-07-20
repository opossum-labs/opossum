use opossum_core::types::api_types::{LoadDocumentResponse, PositionUpdate, UndoRedoResponse};
use uuid::Uuid;

use crate::HTTP_API_CLIENT;

/// Send a request to delete the current scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the request fails (e.g. the scenery is not valid)
pub async fn delete_document() -> Result<String, String> {
    HTTP_API_CLIENT()
        .delete::<String, String>("/api/document", String::new())
        .await
}
pub async fn get_document_root_uuid() -> Result<Uuid, String> {
    HTTP_API_CLIENT().get("/api/document/root_uuid").await
}
/// Send request to receive the `OPM` file representation (as string) of the scenery.
/// This function is used to while saving a model file to disk.
///
/// # Errors
///
/// This function will return an error if .
pub async fn get_document() -> Result<String, String> {
    HTTP_API_CLIENT().get_raw("/api/document").await
}
/// Send request to load a scenery from an `OPM` file (string).
///
/// # Errors
///
/// This function will return an error if
/// - the `OPM` file cannot be parsed
/// - the scenery cannot be constructed from the file data.
pub async fn put_document(opm_string: String) -> Result<LoadDocumentResponse, String> {
    // Use regular .put() instead of .put_string() to deserialize the JSON response
    HTTP_API_CLIENT()
        .put_string_receive_json::<LoadDocumentResponse>("/api/document", opm_string)
        .await
}

/// Undo the last checkpointed document edit.
///
/// # Errors
///
/// This function will return an error if there is nothing to undo, or the request fails.
pub async fn undo_document() -> Result<UndoRedoResponse, String> {
    HTTP_API_CLIENT().post("/api/document/undo", ()).await
}

/// Redo the last undone document edit.
///
/// # Errors
///
/// This function will return an error if there is nothing to redo, or the request fails.
pub async fn redo_document() -> Result<UndoRedoResponse, String> {
    HTTP_API_CLIENT().post("/api/document/redo", ()).await
}

/// Batch-update the GUI positions of several nodes/analyzers in one undo step.
///
/// # Errors
///
/// This function will return an error if the request fails.
pub async fn patch_positions(updates: Vec<PositionUpdate>) -> Result<(), String> {
    HTTP_API_CLIENT()
        .patch("/api/document/positions", updates)
        .await
}
