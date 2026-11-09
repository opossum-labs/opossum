//! Event types reported by [`crate::GlbViewer`] to the caller.

// ─── PickEvent ───────────────────────────────────────────────────────────────

/// A left-click that was not an orbit drag.
///
/// Delivered to the `on_pick` handler. `id == None` means the user clicked empty space;
/// `point == None` also occurs when no object was hit.
#[derive(Clone, Debug, PartialEq)]
pub struct PickEvent {
    /// The [`crate::GlbObject::id`] of the hit object, or `None` for a miss.
    pub id: Option<String>,
    /// Name of the concrete glTF mesh under the cursor, if the file provides one.
    pub mesh_name: Option<String>,
    /// World coordinates of the intersection point, `None` for a miss.
    pub point: Option<[f32; 3]>,
    /// Was the Shift key held?
    pub shift: bool,
    /// Was the Ctrl or Meta key held?
    pub ctrl: bool,
    /// Was the Alt key held?
    pub alt: bool,
}

// ─── ViewerEvent ─────────────────────────────────────────────────────────────

/// Everything the renderer reports besides picks.
///
/// Delivered to the optional `on_event` handler.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ViewerEvent {
    /// three.js is initialised, the canvas has a WebGL context, the op pump is running.
    Ready,
    /// A model was successfully loaded and is visible in the scene.
    Loaded {
        /// Caller-assigned ID of the loaded object.
        id: String,
    },
    /// A model could not be loaded; it is not in the scene.
    LoadError {
        /// Caller-assigned ID of the failed object.
        id: String,
        /// Error message from the `GLTFLoader` or Base64 decoding.
        message: String,
    },
    /// A non-recoverable problem (no WebGL, module import failed, invalid op).
    Error {
        /// Description of the error.
        message: String,
    },
}
