use opossum_core::types::api_types::{
    LoadDocumentResponse, PositionUpdate, SceneManifest, UndoRedoResponse, Viewport,
    ViewportChangeRequest,
};
use uuid::Uuid;

use crate::HTTP_API_CLIENT;

/// Send a request to delete the current scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the request fails (e.g. the scenery is not valid)
pub async fn delete_document() -> Result<String, String> {
    HTTP_API_CLIENT().delete::<String>("/api/document").await
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
pub async fn delete_undo_history() -> Result<(), String> {
    HTTP_API_CLIENT().delete("/api/document/undo").await
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

/// Record a canvas viewport change (pan/zoom of a tab), given the viewport `before` and `after` the
/// gesture. `coalesce` = true for scroll-zoom ticks (a whole burst collapses to one undo step
/// backend-side), false for discrete gestures (pan, center, zoom-to-fit) so they stay separate undo
/// steps and never merge with an adjacent zoom. `merge_into_previous` = true folds this change into
/// the immediately preceding edit's undo entry instead of pushing its own (Auto Layout's post-layout
/// fit, so one undo reverts both).
///
/// # Errors
///
/// This function will return an error if the request fails.
pub async fn post_viewport_change(
    before: Viewport,
    after: Viewport,
    coalesce: bool,
    merge_into_previous: bool,
) -> Result<(), String> {
    HTTP_API_CLIENT()
        .post_receive_no_content(
            "/api/document/viewport_change",
            ViewportChangeRequest {
                before,
                after,
                coalesce,
                merge_into_previous,
            },
        )
        .await
}

/// Ask for the list of drawable components of the current model, with their placement.
///
/// This is the cheap half of the 3D view and the one that is refetched whenever the model changes;
/// the geometry of a component is fetched separately, and only when its `geometry` hash says its
/// shape actually changed. See [`crate::components::scene_view::objects_of`]. The axis and the
/// rays are fetched the same way, by the digests the manifest names for them.
///
/// # Arguments
///
/// - `analyzer`: the analyzer to take the placement from, or `None` for the one the model is drawn
///   after by default
/// - `rays`: the most rays drawn and whether each start point shines every wavelength, or `None`
///   while no rays are drawn; only then does the manifest digest the rays, which costs a trace
///
/// # Errors
///
/// This function will return an error if the components cannot be listed. A model that cannot be
/// placed is no error: the manifest's `notice` says why.
pub async fn get_scene_manifest(
    analyzer: Option<Uuid>,
    rays: Option<(usize, bool)>,
) -> Result<SceneManifest, String> {
    let query: Vec<String> = analyzer
        .map(|id| format!("analyzer={id}"))
        .into_iter()
        .chain(rays.map(|(max_rays, full_spectrum)| {
            format!("max_rays={max_rays}&full_spectrum={full_spectrum}")
        }))
        .collect();
    HTTP_API_CLIENT()
        .get(&format!("/api/document/scene/manifest?{}", query.join("&")))
        .await
}

/// The URL the 3D viewer fetches one component's geometry from.
///
/// The `geometry` hash rides along as a query parameter, which is what makes the URL change exactly
/// when the component's shape does. The viewer compares sources by value and reloads a model only
/// when its URL changed, so a component that merely moved keeps this URL and is never refetched.
///
/// The base is a parameter rather than read from [`crate::HTTP_API_CLIENT`] here, because this URL
/// is not fetched by us: the webview loads it. That keeps the route a plain function of its inputs,
/// callable from anywhere, rather than something that only works inside a running Dioxus runtime.
///
/// # Arguments
///
/// - `base_url`: the backend's root, from [`crate::api::http_client::HTTPClient::base_url`]
/// - `uid`: the component's node uuid
/// - `geometry`: the component's geometry hash, from its [`SceneManifest`] entry
///
/// # Returns
///
/// An absolute URL the webview can fetch.
#[must_use]
pub fn scene_node_url(base_url: &str, uid: Uuid, geometry: &str) -> String {
    format!("{base_url}/api/document/scene/node/{uid}.glb?v={geometry}")
}

/// The URL the 3D viewer fetches the model's optical axis from.
///
/// The axis's digest from the [`SceneManifest`] rides along like a component's geometry hash in
/// [`scene_node_url`], so the URL changes exactly when the axis does, and the viewer, which reloads
/// an object exactly when its URL changes, fetches it again only then.
///
/// # Arguments
///
/// - `base_url`: the backend's root, from [`crate::api::http_client::HTTPClient::base_url`]
/// - `version`: the axis's digest, from the manifest's `axis`
/// - `analyzer`: the analyzer the components were placed after, so the axis follows the same one
///
/// # Returns
///
/// An absolute URL the webview can fetch.
#[must_use]
pub fn scene_axis_url(base_url: &str, version: &str, analyzer: Option<Uuid>) -> String {
    format!(
        "{base_url}/api/document/scene/axis.glb?v={version}{}",
        analyzer_param(analyzer)
    )
}

/// The URL the 3D viewer fetches the light of the model's sources from.
///
/// Versioned by the rays' digest like [`scene_axis_url`], and naming the ray settings the manifest
/// digested them for, so the file fetched is the one digested.
///
/// # Arguments
///
/// - `base_url`: the backend's root, from [`crate::api::http_client::HTTPClient::base_url`]
/// - `version`: the rays' digest, from the manifest's `rays`
/// - `max_rays`: the most rays drawn, of all sources together; the backend caps it
/// - `full_spectrum`: whether each start point shines every wavelength rather than one
/// - `analyzer`: the analyzer the components were placed after, so the rays are its light
///
/// # Returns
///
/// An absolute URL the webview can fetch.
#[must_use]
pub fn scene_rays_url(
    base_url: &str,
    version: &str,
    max_rays: usize,
    full_spectrum: bool,
    analyzer: Option<Uuid>,
) -> String {
    format!(
        "{base_url}/api/document/scene/rays.glb?max_rays={max_rays}&full_spectrum={full_spectrum}&v={version}{}",
        analyzer_param(analyzer)
    )
}

/// The query parameter naming `analyzer`, to append after another one; empty for none.
fn analyzer_param(analyzer: Option<Uuid>) -> String {
    analyzer
        .map(|id| format!("&analyzer={id}"))
        .unwrap_or_default()
}
