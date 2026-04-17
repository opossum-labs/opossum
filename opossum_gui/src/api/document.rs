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
        .delete::<String, String>("/api/document/", String::new())
        .await
}
pub async fn get_document_root_uuid() -> Result<Uuid, String> {
    HTTP_API_CLIENT().get("/api/document/root_uuid").await
}