//! Turning a model's scene manifest into the object list the 3D viewer renders.
//!
//! This is the whole seam between what the backend exports and `dioxus_glb_viewer`, which draws it.
//! Neither side knows about the other: the backend states what the components are and where they
//! sit, the viewer states how it wants to be told, and this module carries one into the other.
//!
//! There is deliberately no arithmetic here. The placement is stated in the manifest already in the
//! precision a renderer works in, and the rotation convention belongs to the viewer, because only it
//! knows which Euler order its `Transform` means. What is left is the part that is genuinely this
//! application's own: which component becomes which object, and where its geometry is fetched from.
//!
//! That split is also what makes updates cheap. The viewer diffs the list it is handed and sends
//! only what changed, so as long as a component keeps its `id` and its source URL, moving it costs
//! a transform and no geometry at all.

use dioxus_glb_viewer::{GlbObject, GlbSource, Transform};
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
            transform: Transform::from_quaternion(node.position, node.rotation),
            visible: true,
            // Selection lives with whoever handles the clicks, not with the geometry.
            selected: false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opossum_core::types::api_types::SceneNodeEntry;
    use uuid::Uuid;

    /// One component, as the backend would list it.
    fn entry(uid: Uuid) -> SceneNodeEntry {
        SceneNodeEntry {
            uid,
            name: "lens".to_owned(),
            geometry: "deadbeefdeadbeef".to_owned(),
            position: [0.0, 0.0, 0.25],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// The uuid is what ties an object in the 3D view back to a node in the graph, and the hash is
    /// what decides whether its mesh is fetched again.
    #[test]
    fn every_component_becomes_an_object_carrying_its_uuid_and_hash() {
        let uid = Uuid::new_v4();
        let manifest = SceneManifest {
            nodes: vec![entry(uid)],
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

    /// The placement travels through untouched. Rounding or re-deriving it here would be a second
    /// opinion about where a component sits, and the point of the manifest is that there is only
    /// one.
    #[test]
    fn a_components_placement_is_passed_through_unchanged() {
        let manifest = SceneManifest {
            nodes: vec![entry(Uuid::new_v4())],
        };

        let objects = objects_of(&manifest, "http://localhost:8001");

        assert_same(objects[0].transform.position, [0.0, 0.0, 0.25], "position");
        assert_same(
            objects[0].transform.rotation_euler_xyz,
            [0.0, 0.0, 0.0],
            "rotation",
        );
        assert_same(objects[0].transform.scale, [1.0, 1.0, 1.0], "scale");
    }

    /// Compare three numbers that were only ever copied, never computed.
    fn assert_same(actual: [f32; 3], wanted: [f32; 3], what: &str) {
        let off = actual
            .iter()
            .zip(wanted)
            .map(|(got, want)| (got - want).abs())
            .fold(0.0_f32, f32::max);
        assert!(off < 1e-6, "{what}: {actual:?} instead of {wanted:?}");
    }

    /// Two components keep their own identities, and the order the manifest names them in is the
    /// order they are handed over in — the viewer's diff keys on the id, but a stable order keeps
    /// an op log readable.
    #[test]
    fn several_components_keep_their_order_and_identity() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let manifest = SceneManifest {
            nodes: vec![entry(first), entry(second)],
        };

        let objects = objects_of(&manifest, "http://localhost:8001");

        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].id, first.to_string());
        assert_eq!(objects[1].id, second.to_string());
    }
}
