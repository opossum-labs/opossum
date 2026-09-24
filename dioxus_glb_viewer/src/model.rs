//! Types for the scene description passed to [`crate::GlbViewer`].

use serde::Serialize;
use std::sync::Arc;

// ─── GlbObject ───────────────────────────────────────────────────────────────

/// A model in the scene.
///
/// The `id` is assigned by the caller and is what [`crate::PickEvent::id`] reports back.
/// It is the diff key: a stable `id` with a changed source or transform produces a minimal
/// update op instead of a full teardown-and-reload.
///
/// # Equality
///
/// `GlbObject` derives `PartialEq`. Equality for [`GlbSource::Bytes`] is
/// **pointer identity** (see there). Cloning an `Arc` is free and does not trigger a
/// reload; a new `Arc` from the same bytes does.
#[derive(Clone, Debug, PartialEq)]
pub struct GlbObject {
    /// Caller-assigned identifier. Must be unique within one `Vec<GlbObject>`.
    pub id: String,
    /// Source of the `.glb` bytes.
    pub source: GlbSource,
    /// Position, rotation, and scale in world coordinates.
    pub transform: Transform,
    /// Whether the object is visible. Invisible objects remain in the scene graph.
    pub visible: bool,
    /// Whether the object is highlighted as selected (with a `THREE.BoxHelper`).
    pub selected: bool,
}

// ─── GlbSource ───────────────────────────────────────────────────────────────

/// Source of the `.glb` bytes.
///
/// # Equality
///
/// `GlbSource::Bytes` compares by **pointer identity** (`Arc::ptr_eq`), not by content.
///
/// The props macro runs `self == new` on every parent render. A content comparison would
/// memcmp the entire model — up to ~20 MB per render for a large file. Pointer identity
/// is the correct semantic: a new `Arc`, even from identical bytes, means "new model" and
/// triggers a reload. That is at worst an unnecessary reload, but never a wrong image.
///
/// `GlbSource::Url` compares by content, because `String` is cheap to compare.
#[derive(Clone, Debug)]
pub enum GlbSource {
    /// A URL that the `WebView` can fetch. Use [`manganis::Asset`] for models bundled with
    /// the app.
    ///
    /// # Desktop note
    ///
    /// On the desktop wry/`WebView`2 cannot load `file://` URLs. For arbitrary files from
    /// the filesystem the host app must read the file in Rust and pass
    /// [`GlbSource::Bytes`].
    Url(String),
    /// Raw `.glb` bytes. Transferred to the `WebView` as chunked Base64 over the eval
    /// channel.
    Bytes(Arc<[u8]>),
}

impl PartialEq for GlbSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Url(a), Self::Url(b)) => a == b,
            (Self::Bytes(a), Self::Bytes(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

// ─── Transform ───────────────────────────────────────────────────────────────

/// Position, rotation, and scale of an object in world coordinates.
///
/// Rotation is **intrinsic Euler XYZ in radians**, matching the default order of
/// `THREE.Euler`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// Translation in world coordinates (X, Y, Z).
    pub position: [f32; 3],
    /// Intrinsic Euler-XYZ rotation in radians.
    pub rotation_euler_xyz: [f32; 3],
    /// Scale factors (X, Y, Z). `[1.0, 1.0, 1.0]` = no scaling.
    pub scale: [f32; 3],
}

impl Default for Transform {
    /// Identity transform: no offset, no rotation, unit scale.
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation_euler_xyz: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

// ─── Environment ─────────────────────────────────────────────────────────────

/// The surroundings a scene is lit and reflected by.
///
/// This is not decoration. glTF's `KHR_materials_transmission` — what a refractive material is
/// exported as — renders what is *behind* the surface, and with nothing around the scene there is
/// nothing to refract: the material comes out black. A lens therefore needs an environment to look
/// like glass rather than like a hole.
///
/// Mirrors and other metallic materials have the same problem in a milder form: without an
/// environment they reflect nothing and read as flat grey.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    /// No environment map. Transmissive and metallic materials render black.
    ///
    /// The default, because it is what the renderer did before this option existed and because
    /// generating the map costs a little time and memory at start-up.
    #[default]
    None,
    /// A small neutral room, the three.js `RoomEnvironment`.
    ///
    /// Grey walls with a few bright panels — enough for glass to refract something and for metal to
    /// have highlights, without tinting the model any particular colour.
    Room,
}
// ─── ViewerOptions ───────────────────────────────────────────────────────────

/// Scene-wide settings for the viewer.
///
/// Changes are sent live as a `set_options` op to the renderer. None of these options
/// touch the camera.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewerOptions {
    /// Background colour, any CSS colour (e.g. `"#1e1e1e"` or `"transparent"`).
    pub background: String,
    /// Whether a reference grid (`GridHelper`) is shown.
    pub grid: bool,
    /// Ambient light intensity (0.0–2.0, default 0.8).
    pub ambient_intensity: f32,
    /// Main (directional) light intensity (default 1.2).
    pub directional_intensity: f32,
    /// CSS colour of the selection `BoxHelper` (e.g. `"#00aaff"`).
    pub selection_color: String,
    /// Whether the scene is automatically framed on the **first** successful model load.
    /// All later adds/removes/replaces leave the camera exactly where the user put it.
    pub fit_on_first_load: bool,
    /// Initial camera position before the first auto-fit (X, Y, Z in world coordinates).
    pub initial_camera: [f32; 3],
    /// Initial camera target before the first auto-fit (X, Y, Z in world coordinates).
    pub initial_target: [f32; 3],
    /// The surroundings the scene is lit and reflected by.
    ///
    /// Needed for refractive and metallic materials to render as anything but black; see
    /// [`Environment`].
    pub environment: Environment,
    /// Vertical field of view in degrees.
    pub fov_degrees: f32,
}

impl Default for ViewerOptions {
    fn default() -> Self {
        Self {
            background: "#1e1e1e".into(),
            grid: true,
            ambient_intensity: 0.8,
            directional_intensity: 1.2,
            selection_color: "#00aaff".into(),
            fit_on_first_load: true,
            initial_camera: [5.0, 3.0, 5.0],
            initial_target: [0.0, 0.0, 0.0],
            environment: Environment::default(),
            fov_degrees: 50.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use core::f32;

    use super::*;

    #[test]
    fn glb_source_url_equality() {
        let a = GlbSource::Url("http://example.com/a.glb".into());
        let b = GlbSource::Url("http://example.com/a.glb".into());
        let c = GlbSource::Url("http://example.com/b.glb".into());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn glb_source_bytes_pointer_equality() {
        let data: Arc<[u8]> = Arc::from(vec![1u8, 2, 3].as_slice());
        let clone = Arc::clone(&data);
        let new_arc: Arc<[u8]> = Arc::from(vec![1u8, 2, 3].as_slice()); // identical bytes, new Arc
        assert_eq!(GlbSource::Bytes(data.clone()), GlbSource::Bytes(clone)); // same Arc → equal
        assert_ne!(GlbSource::Bytes(data), GlbSource::Bytes(new_arc)); // new Arc → unequal → reload
    }

    #[test]
    fn glb_source_cross_variant_not_equal() {
        let url = GlbSource::Url("x".into());
        let bytes: Arc<[u8]> = Arc::from(b"x".as_slice());
        assert_ne!(url, GlbSource::Bytes(bytes));
    }

    #[test]
    fn transform_default_is_identity() {
        let t = Transform::default();
        assert!(t.position[0].abs() < f32::EPSILON);
        assert!(t.position[1].abs() < f32::EPSILON);
        assert!(t.position[2].abs() < f32::EPSILON);
        assert!(t.rotation_euler_xyz[0].abs() < f32::EPSILON);
        assert!(t.rotation_euler_xyz[1].abs() < f32::EPSILON);
        assert!(t.rotation_euler_xyz[2].abs() < f32::EPSILON);
        assert!((t.scale[0] - 1.).abs() < f32::EPSILON);
        assert!((t.scale[1] - 1.).abs() < f32::EPSILON);
        assert!((t.scale[2] - 1.).abs() < f32::EPSILON);
    }
}
