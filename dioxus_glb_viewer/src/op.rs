//! Op protocol between Rust and the JavaScript viewer module.
//!
//! **Rust → JS** — Ops are sent as JSON over `document::eval` (`serde` tag = `"op"`).
//! **JS → Rust** — Events arrive via `dioxus.send(...)` (tag = `"event"`).

use crate::model::{Transform, ViewerOptions};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ─── Rust → JS ───────────────────────────────────────────────────────────────

/// Minimal op list the Rust diff sends to the JS renderer.
///
/// Each op is transmitted as a single JSON object via `eval.send()`; never batched.
/// Byte payloads are streamed first as `BytesBegin` + `BytesChunk*` and then referenced
/// by an `Add`/`Reload` via `WireSource::Bytes { key, .. }`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Load a new model into the scene. The camera is not touched.
    Add {
        id: String,
        source: WireSource,
        transform: Transform,
        visible: bool,
        selected: bool,
    },
    /// Dispose the existing model and replace it with a new source.
    /// Transform/visibility/selection travel in the same op so the new model never
    /// appears for a frame in the wrong pose.
    Reload {
        id: String,
        source: WireSource,
        transform: Transform,
        visible: bool,
        selected: bool,
    },
    /// Remove a model from the scene and release its GPU resources.
    Remove { id: String },
    /// Update the position/rotation/scale of an existing model.
    SetTransform { id: String, transform: Transform },
    /// Change the visibility of a model (it stays in the scene graph).
    SetVisible { id: String, visible: bool },
    /// Toggle the selection highlight (`BoxHelper`) on or off.
    SetSelected { id: String, selected: bool },
    /// Update scene-wide options (background, lights, grid, …). None of these options
    /// touch the camera.
    SetOptions { options: ViewerOptions },
    /// Frame the scene so all currently visible models fit.
    /// **The only op that moves the camera — reachable only via `ViewerHandle`.**
    FitView,
    /// Reset the camera to the pose configured in `ViewerOptions`.
    /// **The only other op that moves the camera — reachable only via `ViewerHandle`.**
    ResetCamera,
    /// Remove all models and release their GPU resources.
    Clear,
    /// Shut down the renderer (RAF loop, WebGL context, all listeners).
    /// Sent from `use_drop`.
    Destroy,
    // --- Transport ops, emitted only by `wire::messages`, never by `diff` --------
    /// Announces a byte stream. `key` identifies the buffer; `chunks` says how many
    /// `BytesChunk` messages follow.
    BytesBegin { key: String, chunks: usize },
    /// One chunk of the Base64-encoded `.glb` buffer.
    BytesChunk {
        key: String,
        seq: usize,
        b64: String,
    },
}

/// Source specification as transmitted over the JS channel.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WireSource {
    /// The `WebView` fetches the URL directly.
    Url { url: String },
    /// The byte buffer was streamed beforehand via `BytesBegin`/`BytesChunk`.
    Bytes {
        key: String,
        /// Payload that `wire::messages` streams ahead of the op carrying this source.
        /// Travels with the op so it can never go missing; never serialised.
        #[serde(skip)]
        data: Arc<[u8]>,
    },
}

// ─── JS → Rust ───────────────────────────────────────────────────────────────

/// Event reported by the JavaScript viewer via `dioxus.send(...)`.
///
/// Received in `viewer.rs` via `eval.recv::<WireEvent>()` and translated into
/// [`crate::event::ViewerEvent`] / [`crate::event::PickEvent`].
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WireEvent {
    /// three.js is up and running, the op pump is active.
    Ready,
    /// Model loaded successfully.
    Loaded { id: String },
    /// Model could not be loaded.
    LoadError { id: String, message: String },
    /// Non-recoverable error.
    Error { message: String },
    /// Click result (including a miss).
    Pick {
        id: Option<String>,
        mesh_name: Option<String>,
        point: Option<[f32; 3]>,
        shift: bool,
        ctrl: bool,
        alt: bool,
    },
}

// ─── Serde compatibility for transport types ──────────────────────────────────

use serde::ser::SerializeStruct;

impl Serialize for Transform {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("Transform", 3)?;
        st.serialize_field("position", &self.position)?;
        st.serialize_field("rotation_euler_xyz", &self.rotation_euler_xyz)?;
        st.serialize_field("scale", &self.scale)?;
        st.end()
    }
}

impl Serialize for ViewerOptions {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Destructure exhaustively (no `..`) so that a new field added to the struct causes a
        // compile error here until it is also emitted on the wire.  The compiler becomes the guard
        // that `viewer_options_put_every_field_on_the_wire` would otherwise miss.
        let Self {
            background,
            grid,
            ambient_intensity,
            directional_intensity,
            selection_color,
            fit_on_first_load,
            initial_camera,
            initial_target,
            environment,
            fov_degrees,
            orientation_gizmo,
        } = self;
        let mut st = s.serialize_struct("ViewerOptions", 11)?;
        st.serialize_field("background", background)?;
        st.serialize_field("grid", grid)?;
        st.serialize_field("ambient_intensity", ambient_intensity)?;
        st.serialize_field("directional_intensity", directional_intensity)?;
        st.serialize_field("selection_color", selection_color)?;
        st.serialize_field("fit_on_first_load", fit_on_first_load)?;
        st.serialize_field("initial_camera", initial_camera)?;
        st.serialize_field("initial_target", initial_target)?;
        st.serialize_field("environment", environment)?;
        st.serialize_field("fov_degrees", fov_degrees)?;
        st.serialize_field("orientation_gizmo", orientation_gizmo)?;
        st.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Environment;

    /// Every field of [`ViewerOptions`] has to reach `viewer.js`. The `Serialize` impl above is
    /// written out by hand and states its own field count, so a field added to the struct is
    /// silently dropped from the wire unless it is added here as well — and an option that never
    /// arrives looks exactly like an option the renderer ignores. This is what notices.
    #[test]
    fn viewer_options_put_every_field_on_the_wire() {
        let json = serde_json::to_value(ViewerOptions::default()).expect("options serialise");
        let mut keys: Vec<&str> = json
            .as_object()
            .expect("options are a JSON object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "ambient_intensity",
                "background",
                "directional_intensity",
                "environment",
                "fit_on_first_load",
                "fov_degrees",
                "grid",
                "initial_camera",
                "initial_target",
                "orientation_gizmo",
                "selection_color",
            ]
        );
    }

    /// `orientation_gizmo` was missing from the hand-written `Serialize` impl when it was first
    /// added to the struct, so `viewer.js` read `undefined` and never created the `ViewHelper`.
    /// This test exists so the same regression cannot recur silently: it serialises
    /// `ViewerOptions` with the gizmo enabled and asserts the JSON value is exactly `true`.
    #[test]
    fn the_orientation_gizmo_reaches_the_wire() {
        let opts = ViewerOptions {
            orientation_gizmo: true,
            ..ViewerOptions::default()
        };
        let json = serde_json::to_value(opts).expect("options serialise");
        assert_eq!(json["orientation_gizmo"], true);
    }

    /// `viewer.js` switches on these exact strings, so renaming a variant silently turns the
    /// environment off rather than failing.
    #[test]
    fn environment_serialises_as_the_names_the_renderer_switches_on() {
        assert_eq!(
            serde_json::to_value(Environment::None).expect("serialises"),
            "none"
        );
        assert_eq!(
            serde_json::to_value(Environment::Room).expect("serialises"),
            "room"
        );
    }
}
