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

use dioxus_glb_viewer::{GlbObject, GlbSource, PickEvent, Transform};
use opossum_core::types::api_types::SceneManifest;
use uuid::Uuid;

use crate::{api::scene_node_url, components::scenery_editor::GraphsWorkspaceAction};

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

/// Turn a click in the 3D view into the action that reveals the node it hit.
///
/// The pick only reports which object was hit; the graph it lives in comes from looking the uid
/// back up in the manifest, which is where the backend filled it in during the same walk that
/// found the component in the first place. The action never brings the node's graph tab to the
/// front - the whole point of clicking in the 3D view is to keep looking at it.
///
/// # Arguments
///
/// - `picked`: what the viewer reported for the click
/// - `manifest`: the components of the model, as the backend listed them
///
/// # Returns
///
/// The action that selects the node and opens its properties, or `None` for a miss, or a pick
/// whose id the manifest no longer lists (a stale click racing a refresh).
pub fn reveal_action_for_pick(
    picked: &PickEvent,
    manifest: &SceneManifest,
) -> Option<GraphsWorkspaceAction> {
    let uid: Uuid = picked.id.as_ref()?.parse().ok()?;
    let group = manifest.nodes.iter().find(|node| node.uid == uid)?.group;
    Some(GraphsWorkspaceAction::RevealNode {
        node_id: uid,
        graph_id: group,
        bring_to_front: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use opossum_core::types::api_types::SceneNodeEntry;

    /// One component, as the backend would list it.
    fn entry(uid: Uuid) -> SceneNodeEntry {
        SceneNodeEntry {
            uid,
            group: Uuid::new_v4(),
            name: "lens".to_owned(),
            geometry: "deadbeefdeadbeef".to_owned(),
            position: [0.0, 0.0, 0.25],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// A pick, the way `dioxus_glb_viewer` reports it. Only `id` matters to
    /// [`reveal_action_for_pick`]; the rest is never read.
    fn pick(id: Option<Uuid>) -> PickEvent {
        PickEvent {
            id: id.map(|uid| uid.to_string()),
            mesh_name: None,
            point: None,
            shift: false,
            ctrl: false,
            alt: false,
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

    /// A hit reveals the node in the graph the manifest says it actually lives in, and never brings
    /// that graph's tab in front of the 3D view - the whole point of clicking there in the first
    /// place.
    #[test]
    fn a_hit_reveals_the_node_without_leaving_the_3d_view() {
        let uid = Uuid::new_v4();
        let manifest = SceneManifest {
            nodes: vec![entry(uid)],
        };

        let action = reveal_action_for_pick(&pick(Some(uid)), &manifest)
            .expect("a hit on a listed component reveals it");

        let GraphsWorkspaceAction::RevealNode {
            node_id,
            graph_id,
            bring_to_front,
        } = action
        else {
            panic!("a 3D pick must reveal the node it hit, not any other action");
        };
        assert_eq!(node_id, uid);
        assert_eq!(graph_id, manifest.nodes[0].group);
        assert!(
            !bring_to_front,
            "a 3D pick must not bring the graph tab in front of the 3D view"
        );
    }

    /// Clicking empty space reports no id and must reveal nothing.
    #[test]
    fn a_miss_reveals_nothing() {
        let manifest = SceneManifest {
            nodes: vec![entry(Uuid::new_v4())],
        };

        assert!(reveal_action_for_pick(&pick(None), &manifest).is_none());
    }

    /// A click can race a manifest refresh and report an id the manifest just stopped listing; that
    /// must not be turned into an action for a node that may not even exist any more.
    #[test]
    fn a_stale_id_reveals_nothing() {
        let manifest = SceneManifest {
            nodes: vec![entry(Uuid::new_v4())],
        };

        assert!(reveal_action_for_pick(&pick(Some(Uuid::new_v4())), &manifest).is_none());
    }
}
