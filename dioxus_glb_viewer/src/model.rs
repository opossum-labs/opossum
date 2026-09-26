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

impl Transform {
    /// Build a transform from a position and a rotation given as a quaternion.
    ///
    /// glTF states rotations as quaternions, and so does most 3D maths — but
    /// [`Self::rotation_euler_xyz`] is Euler angles in three.js's order, and Euler angles mean
    /// nothing without knowing that order. Which order this component uses is *this crate's*
    /// business, so the conversion lives here rather than in every caller that happens to hold a
    /// quaternion.
    ///
    /// The decomposition is not unique, and at a quarter turn about Y the X and Z axes line up and
    /// the split between them becomes arbitrary. The angles that come out are then only one of
    /// several valid answers, but they always rebuild the rotation that went in — which is the only
    /// thing the renderer is asked to reproduce.
    ///
    /// # Arguments
    ///
    /// - `position`: translation in world coordinates
    /// - `rotation`: a unit quaternion in the order `(x, y, z, w)`
    ///
    /// # Returns
    ///
    /// The transform, with unit scale.
    #[must_use]
    pub fn from_quaternion(position: [f32; 3], rotation: [f32; 4]) -> Self {
        Self {
            position,
            rotation_euler_xyz: euler_xyz_of(rotation),
            scale: [1.0, 1.0, 1.0],
        }
    }
}

/// The Euler angles, in three.js's default XYZ order, of a rotation given as a quaternion.
///
/// This reproduces `THREE.Euler.setFromRotationMatrix(m, 'XYZ')` element for element, working from
/// the rotation matrix the quaternion describes. That order composes as `R = Rx * Ry * Rz`; other
/// engines pick other orders, which is exactly why the angles alone are not a portable way to state
/// a rotation.
// Written as the textbook quaternion-to-matrix formula rather than as fused multiply-adds. The
// accuracy clippy is after is not worth anything here - the angles feed a renderer - while being
// able to check these nine entries against a reference by eye very much is.
#[allow(clippy::suboptimal_flops)]
fn euler_xyz_of(rotation: [f32; 4]) -> [f32; 3] {
    let [qx, qy, qz, qw] = rotation;
    // The rotation matrix of a unit quaternion, in the same element order three.js reads it in:
    // `m[row][column]`.
    let m = [
        [
            1.0 - 2.0 * (qy * qy + qz * qz),
            2.0 * (qx * qy - qz * qw),
            2.0 * (qx * qz + qy * qw),
        ],
        [
            2.0 * (qx * qy + qz * qw),
            1.0 - 2.0 * (qx * qx + qz * qz),
            2.0 * (qy * qz - qx * qw),
        ],
        [
            2.0 * (qx * qz - qy * qw),
            2.0 * (qy * qz + qx * qw),
            1.0 - 2.0 * (qx * qx + qy * qy),
        ],
    ];

    let about_y = m[0][2].clamp(-1.0, 1.0).asin();
    // Past this the Y rotation is so close to a quarter turn that the other two axes have collapsed
    // onto each other. Pinning Z and giving X the whole remainder keeps the rotation right; three.js
    // uses the same threshold and the same fallback.
    let (about_x, about_z) = if m[0][2].abs() < 0.999_999_9 {
        ((-m[1][2]).atan2(m[2][2]), (-m[0][1]).atan2(m[0][0]))
    } else {
        (m[2][1].atan2(m[1][1]), 0.0)
    };
    [about_x, about_y, about_z]
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
// ─── Ground ──────────────────────────────────────────────────────────────────

/// A flat floor under the scene that reaches to the horizon.
///
/// The viewer knows nothing about what the floor shows: the host hands it an image and says how
/// large one copy of it is in the world, and the viewer tiles it. It is drawn as a plane that
/// follows the camera — moved in whole tiles, so the pattern stays fixed in the world — and fades
/// into the background towards its rim, so it never shows an edge.
///
/// The ground is not a model. [`ViewerHandle::fit_view`](crate::ViewerHandle::fit_view) does not
/// frame it, and a click never hits it.
#[derive(Clone, Debug, PartialEq)]
pub struct Ground {
    /// Height of the floor (world y).
    pub height: f32,
    /// URL of the image tiled across the floor. The webview fetches it itself, like a model URL.
    pub tile_url: String,
    /// Edge length of one copy of the image, in world units. Must be positive and finite; a ground
    /// with any other tile size is not drawn and reported as an error instead.
    pub tile_size: f32,
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
    /// Whether a corner orientation gizmo (`ViewHelper`) is shown.
    ///
    /// The gizmo renders in the bottom-right corner of the canvas and shows the current camera
    /// orientation as coloured axes. Clicking an axis snaps the camera to that view. Defaults
    /// to `false` so that viewers that do not need the gizmo pay neither the import cost nor
    /// the per-frame overlay render.
    pub orientation_gizmo: bool,
    /// A floor under the scene that reaches to the horizon, or `None` for no floor. See [`Ground`].
    pub ground: Option<Ground>,
}

impl Default for ViewerOptions {
    fn default() -> Self {
        Self {
            background: "#6c6c6c".into(),
            grid: true,
            ambient_intensity: 0.8,
            directional_intensity: 3.,
            selection_color: "#00aaff".into(),
            fit_on_first_load: true,
            initial_camera: [0.5, 0.3, 0.5],
            initial_target: [0.0, 0.0, 0.0],
            environment: Environment::default(),
            fov_degrees: 50.0,
            orientation_gizmo: false,
            ground: None,
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

    /// Compose a quaternion from Euler XYZ angles exactly as `THREE.Quaternion.setFromEuler` does,
    /// so the conversion is checked against the convention it has to agree with rather than against
    /// itself.
    // Transcribed from `THREE.Quaternion.setFromEuler`; rewriting it as fused multiply-adds would
    // make it unrecognisable against the source it has to match.
    #[allow(clippy::suboptimal_flops)]
    fn quaternion_of_euler_xyz(angles: [f32; 3]) -> [f32; 4] {
        let (s1, c1) = (angles[0] * 0.5).sin_cos();
        let (s2, c2) = (angles[1] * 0.5).sin_cos();
        let (s3, c3) = (angles[2] * 0.5).sin_cos();
        [
            s1 * c2 * c3 + c1 * s2 * s3,
            c1 * s2 * c3 - s1 * c2 * s3,
            c1 * c2 * s3 + s1 * s2 * c3,
            c1 * c2 * c3 - s1 * s2 * s3,
        ]
    }

    /// The angles have to rebuild the rotation they came from. Comparing the angles themselves would
    /// be the wrong test: the decomposition is not unique, and reproducing the rotation is the only
    /// thing that decides what ends up on screen. Two unit quaternions describe the same rotation
    /// when they agree up to sign, so the magnitude of their dot product is what is checked.
    fn assert_round_trips(angles: [f32; 3], what: &str) {
        let wanted = quaternion_of_euler_xyz(angles);
        let rebuilt = quaternion_of_euler_xyz(euler_xyz_of(wanted));
        let alignment = wanted
            .iter()
            .zip(rebuilt)
            .map(|(l, r)| l * r)
            .sum::<f32>()
            .abs();
        assert!(
            (alignment - 1.0).abs() < 1e-4,
            "{what}: the angles rebuild a different rotation ({wanted:?} vs {rebuilt:?})"
        );
    }

    #[test]
    fn a_turn_about_each_single_axis_round_trips() {
        for turn in [0.3_f32, -1.2, core::f32::consts::FRAC_PI_2, 2.9] {
            assert_round_trips([turn, 0.0, 0.0], "a turn about x");
            assert_round_trips([0.0, turn, 0.0], "a turn about y");
            assert_round_trips([0.0, 0.0, turn], "a turn about z");
        }
    }

    #[test]
    fn a_turn_about_several_axes_at_once_round_trips() {
        assert_round_trips([0.4, -0.9, 1.7], "a rotation about all three axes");
        assert_round_trips([-2.1, 0.5, 0.8], "another rotation about all three axes");
    }

    /// A mirror folding the beam by ninety degrees sits exactly on the singularity of the XYZ
    /// decomposition, and it is one of the most ordinary things in an optical setup - so the case
    /// that is easiest to get wrong is also the one most likely to occur.
    #[test]
    fn a_quarter_turn_about_y_round_trips_although_the_angles_are_ambiguous() {
        for sign in [1.0_f32, -1.0] {
            let quarter = sign * core::f32::consts::FRAC_PI_2;
            assert_round_trips([0.0, quarter, 0.0], "a folding mirror at a quarter turn");
            assert_round_trips(
                [0.6, quarter, 0.0],
                "a quarter turn about y, tilted about x",
            );
        }
    }

    /// A component that never moved must produce no update at all, and the viewer's diff compares
    /// transforms by value - so the identity has to come out as exactly zero, not as some other set
    /// of angles that happens to compose to it.
    #[test]
    fn no_rotation_is_no_rotation() {
        let transform = Transform::from_quaternion([0.0; 3], [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(transform, Transform::default());
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
