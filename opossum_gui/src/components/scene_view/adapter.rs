//! Turning a model's scene manifest into the object list the 3D viewer renders.
//!
//! This is the whole seam between `optoscene`, which the backend exports the geometry with, and
//! `dioxus_glb_viewer`, which draws it. Neither knows about the other: the backend states what the
//! components are and where they sit, and this module says it in the viewer's own vocabulary.
//!
//! Keeping the two apart is also what makes updates cheap. The viewer diffs the list it is handed
//! and sends only what changed, so as long as a component keeps its `id` and its source URL, moving
//! it costs a transform and no geometry at all.

use dioxus_glb_viewer::{GlbObject, GlbSource, Transform};
use nalgebra::{Quaternion, UnitQuaternion};
use opossum_core::types::api_types::SceneManifest;

use crate::api::scene_node_url;

/// Describe a model's components the way the 3D viewer wants them.
///
/// The node uuid becomes the object id, which is what a click in the 3D view reports back and what
/// ties it to a node in the graph. The geometry is referenced by URL rather than sent along: the
/// webview fetches it itself, so meshes never travel through the JavaScript bridge.
///
/// # Arguments
///
/// - `manifest`: the components of the model, as the backend listed them
/// - `base_url`: the backend's root, which the geometry URLs are built from
///
/// # Returns
///
/// One object per component, in the order the manifest names them.
pub fn objects_of(manifest: &SceneManifest, base_url: &str) -> Vec<GlbObject> {
    manifest
        .nodes
        .iter()
        .map(|node| GlbObject {
            id: node.uid.to_string(),
            // Only the hash in the URL decides whether the viewer refetches: the URL is compared by
            // value, so a component that merely moved keeps it and is never reloaded.
            source: GlbSource::Url(scene_node_url(base_url, node.uid, &node.geometry)),
            transform: Transform {
                position: metres_to_f32(node.position),
                rotation_euler_xyz: euler_xyz_of(node.rotation),
                scale: [1.0, 1.0, 1.0],
            },
            visible: true,
            // Selection lives with whoever handles the clicks, not with the geometry.
            selected: false,
        })
        .collect()
}

/// Narrow a position to what a renderer works in.
// Positions arrive relative to the manifest's origin and so stay within the extent of one optical
// setup - metres, not astronomical units. `f32` holds that with room to spare.
#[allow(clippy::cast_possible_truncation)]
const fn metres_to_f32(position: [f64; 3]) -> [f32; 3] {
    [position[0] as f32, position[1] as f32, position[2] as f32]
}

/// Convert a quaternion into the Euler angles the viewer expects.
///
/// The viewer states its rotations as **intrinsic Euler XYZ**, the default of `THREE.Euler`, which
/// composes as `R = Rx * Ry * Rz`. nalgebra's own `euler_angles` returns the *other* convention
/// (`Rz * Ry * Rx`) and would silently mis-orient every component that is turned about more than one
/// axis, so the extraction from `THREE.Euler.setFromRotationMatrix(m, 'XYZ')` is reproduced here
/// instead.
///
/// # Arguments
///
/// - `rotation`: a quaternion in the order `(x, y, z, w)`, as the manifest states it
///
/// # Returns
///
/// The three angles in radians, to be applied in the order X, then Y, then Z.
// The angles are a renderer's input, and a rotation is worth nowhere near `f32`'s precision.
#[allow(clippy::cast_possible_truncation)]
fn euler_xyz_of(rotation: [f64; 4]) -> [f32; 3] {
    let [qx, qy, qz, qw] = rotation;
    let matrix =
        UnitQuaternion::from_quaternion(Quaternion::new(qw, qx, qy, qz)).to_rotation_matrix();

    let about_y = matrix[(0, 2)].clamp(-1.0, 1.0).asin();
    // Near a quarter turn about Y the other two axes line up and the split between them is
    // arbitrary. Pinning Z and putting the whole remainder into X keeps the *rotation* right, which
    // is all that is asked of it - a folding mirror at 90 degrees lands exactly here.
    let (about_x, about_z) = if matrix[(0, 2)].abs() < 0.999_999_9 {
        (
            (-matrix[(1, 2)]).atan2(matrix[(2, 2)]),
            (-matrix[(0, 1)]).atan2(matrix[(0, 0)]),
        )
    } else {
        (matrix[(2, 1)].atan2(matrix[(1, 1)]), 0.0)
    };
    [about_x as f32, about_y as f32, about_z as f32]
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::{Rotation3, Vector3};
    use opossum_core::types::api_types::SceneNodeEntry;
    use uuid::Uuid;

    /// Build a rotation the way `THREE.Euler` with the default XYZ order does, so that the
    /// conversion is checked against the thing it has to agree with rather than against itself.
    fn three_js_xyz(angles: [f32; 3]) -> Rotation3<f64> {
        Rotation3::from_axis_angle(&Vector3::x_axis(), f64::from(angles[0]))
            * Rotation3::from_axis_angle(&Vector3::y_axis(), f64::from(angles[1]))
            * Rotation3::from_axis_angle(&Vector3::z_axis(), f64::from(angles[2]))
    }

    /// A quaternion `(x, y, z, w)` as the manifest states it, from a rotation.
    fn quaternion_of(rotation: Rotation3<f64>) -> [f64; 4] {
        let q = UnitQuaternion::from_rotation_matrix(&rotation);
        [q.i, q.j, q.k, q.w]
    }

    /// The angles have to reproduce the rotation they came from. The decomposition itself is not
    /// unique, so comparing angles would be the wrong test - comparing the rotation is the promise
    /// that actually matters to what ends up on screen.
    fn assert_round_trips(rotation: Rotation3<f64>, what: &str) {
        let recomposed = three_js_xyz(euler_xyz_of(quaternion_of(rotation)));
        let error = (recomposed.matrix() - rotation.matrix()).abs().max();
        assert!(
            error < 1e-6,
            "{what}: the angles rebuild a different rotation (worst element off by {error})"
        );
    }

    #[test]
    fn a_turn_about_each_single_axis_round_trips() {
        for (axis, name) in [
            (Vector3::x_axis(), "x"),
            (Vector3::y_axis(), "y"),
            (Vector3::z_axis(), "z"),
        ] {
            for turn in [0.3_f64, -1.2, std::f64::consts::FRAC_PI_2, 2.9] {
                assert_round_trips(
                    Rotation3::from_axis_angle(&axis, turn),
                    &format!("{turn} rad about {name}"),
                );
            }
        }
    }

    #[test]
    fn a_turn_about_several_axes_at_once_round_trips() {
        assert_round_trips(
            three_js_xyz([0.4, -0.9, 1.7]),
            "a rotation about all three axes",
        );
        assert_round_trips(
            Rotation3::from_euler_angles(0.8, 0.2, -1.1),
            "a rotation built the other way round",
        );
    }

    /// A mirror folding the beam by ninety degrees sits exactly on the singularity of the XYZ
    /// decomposition, and it is one of the most ordinary things in an optical setup - so the case
    /// that is easiest to get wrong is also the one most likely to occur.
    #[test]
    fn a_quarter_turn_about_y_round_trips_although_the_angles_are_ambiguous() {
        for sign in [1.0_f64, -1.0] {
            let quarter = sign * std::f64::consts::FRAC_PI_2;
            assert_round_trips(
                Rotation3::from_axis_angle(&Vector3::y_axis(), quarter),
                "a folding mirror at a quarter turn",
            );
            assert_round_trips(
                Rotation3::from_axis_angle(&Vector3::x_axis(), 0.6)
                    * Rotation3::from_axis_angle(&Vector3::y_axis(), quarter),
                "a quarter turn about y with a tilt about x",
            );
        }
    }

    /// The identity has to come out as the identity, not as some equivalent set of angles that
    /// happens to compose to it - the viewer's diff compares transforms by value, so a component
    /// that never moved must produce no update at all.
    #[test]
    fn no_rotation_is_no_rotation() {
        let angles = euler_xyz_of([0.0, 0.0, 0.0, 1.0]);
        assert!(
            angles.iter().all(|angle| angle.abs() < f32::EPSILON),
            "the identity came out as {angles:?}"
        );
    }

    /// The uuid is what ties an object in the 3D view back to a node in the graph, and the hash is
    /// what decides whether its mesh is fetched again.
    #[test]
    fn every_component_becomes_an_object_carrying_its_uuid_and_hash() {
        let uid = Uuid::new_v4();
        let manifest = SceneManifest {
            origin: [0.0; 3],
            nodes: vec![SceneNodeEntry {
                uid,
                name: "lens".to_owned(),
                geometry: "deadbeefdeadbeef".to_owned(),
                position: [0.0, 0.0, 0.25],
                rotation: [0.0, 0.0, 0.0, 1.0],
            }],
        };

        let objects = objects_of(&manifest, "http://localhost:8001");

        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, uid.to_string());
        assert!(objects[0].visible);
        assert!(!objects[0].selected);
        let GlbSource::Url(url) = &objects[0].source else {
            panic!("a component's geometry is fetched by url, not carried as bytes");
        };
        assert!(
            url.contains(&uid.to_string()),
            "{url} does not name the node"
        );
        assert!(
            url.ends_with("v=deadbeefdeadbeef"),
            "{url} has no geometry hash"
        );
    }
}
