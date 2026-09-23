//! Scene diffing and message framing (feature `protocol`).
//!
//! [`full_scene`] wraps a whole scene as one [`Header::FullScene`] message;
//! [`diff`] compares two scenes and produces the minimal set of update messages.
//! Each [`SceneMessage`] can be turned into a wire frame with
//! [`SceneMessage::to_frame`].

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::Hasher;

use nalgebra::Point3;
use optoscene_protocol::{encode, Header, TransformEntry};

use crate::error::Error;
use crate::model::{Layer, SceneNode};
use crate::scene::Scene;
use crate::util::to_f32;

/// The largest translation (meters) and rotation angle (radians) still treated
/// as "unchanged" when diffing node transforms.
const EPSILON: f64 = 1e-9;

/// A header plus its (possibly empty) binary payload, ready to be framed.
pub struct SceneMessage {
    /// The message header.
    pub header: Header,
    /// The GLB payload, or empty for payload-less headers.
    pub payload: Vec<u8>,
}

impl SceneMessage {
    /// Encodes this message into a protocol frame.
    ///
    /// # Returns
    /// The frame bytes.
    ///
    /// # Errors
    /// Returns [`Error::Frame`] if the header cannot be encoded.
    pub fn to_frame(&self) -> Result<Vec<u8>, Error> {
        Ok(encode(&self.header, &self.payload)?)
    }
}

/// Builds a single [`Header::FullScene`] message carrying the whole scene as GLB.
///
/// # Arguments
/// * `scene` — the scene to serialize.
///
/// # Returns
/// The full-scene message.
///
/// # Errors
/// Returns an export error if the scene cannot be serialized to GLB.
pub fn full_scene(scene: &Scene) -> Result<SceneMessage, Error> {
    Ok(SceneMessage {
        header: Header::FullScene,
        payload: scene.to_glb()?,
    })
}

/// Computes the update messages that turn `old` into `new`.
///
/// The rules, applied in order:
/// 1. If the effective origin or the root rotation differ, the result is a
///    single [`Header::FullScene`].
/// 2. The `rays` layer is treated as a whole: if its set of uids, any node's
///    content or any node's transform differs, a [`Header::ReplaceLayer`] for
///    `"rays"` is emitted and those nodes are excluded from rules 3–5.
/// 3. Non-rays uids present only in `old` become one [`Header::RemoveNodes`].
/// 4. Non-rays uids that are new or whose content changed become one
///    [`Header::UpsertNodes`], in `new`'s insertion order.
/// 5. Remaining non-rays nodes whose transform changed become one
///    [`Header::UpdateTransforms`].
///
/// Messages are ordered `RemoveNodes`, `UpsertNodes`, `UpdateTransforms`,
/// `ReplaceLayer`; an unchanged scene yields an empty vector.
///
/// # Arguments
/// * `old` — the previously sent scene.
/// * `new` — the current scene.
///
/// # Returns
/// The ordered update messages.
///
/// # Errors
/// Returns an export error if a payload GLB cannot be serialized.
pub fn diff(old: &Scene, new: &Scene) -> Result<Vec<SceneMessage>, Error> {
    // Rule 1: a changed origin or root rotation forces a full resend.
    let new_origin = new.effective_origin();
    let origin_shift = (old.effective_origin() - new_origin).norm();
    let rotation_change = old
        .options()
        .root_rotation
        .angle_to(&new.options().root_rotation);
    if origin_shift > EPSILON || rotation_change > EPSILON {
        return Ok(vec![full_scene(new)?]);
    }

    // Rays (rule 2) are handled wholesale; everything else by uid.
    let old_non_rays = non_rays_by_uid(old);
    let new_non_rays = non_rays_by_uid(new);

    // Rule 3: non-rays uids present only in `old`, in `old`'s insertion order.
    let removed: Vec<String> = old
        .nodes()
        .iter()
        .filter(|node| node.layer != Layer::Rays)
        .filter(|node| !new_non_rays.contains_key(node.uid.as_str()))
        .map(|node| node.uid.clone())
        .collect();

    // Rules 4 and 5: walk `new` in insertion order.
    let origin = new_origin;
    let mut upserts: Vec<String> = Vec::new();
    let mut transforms: Vec<TransformEntry> = Vec::new();
    for node in new.nodes().iter().filter(|node| node.layer != Layer::Rays) {
        match old_non_rays.get(node.uid.as_str()) {
            None => upserts.push(node.uid.clone()),
            Some(old_node) => {
                if old.node_content_hash(old_node) != new.node_content_hash(node) {
                    upserts.push(node.uid.clone());
                } else if transform_changed(old_node, node) {
                    transforms.push(transform_entry(node, origin));
                }
            }
        }
    }

    let rays_changed = rays_fingerprints(old) != rays_fingerprints(new);

    // Rule 6: the fixed message order.
    let mut messages = Vec::new();
    if !removed.is_empty() {
        messages.push(SceneMessage {
            header: Header::RemoveNodes { uids: removed },
            payload: Vec::new(),
        });
    }
    if !upserts.is_empty() {
        let refs: Vec<&str> = upserts.iter().map(String::as_str).collect();
        let payload = new.nodes_to_glb(&refs)?;
        messages.push(SceneMessage {
            header: Header::UpsertNodes { uids: upserts },
            payload,
        });
    }
    if !transforms.is_empty() {
        messages.push(SceneMessage {
            header: Header::UpdateTransforms { nodes: transforms },
            payload: Vec::new(),
        });
    }
    if rays_changed {
        let payload = new.layer_to_glb(Layer::Rays)?;
        messages.push(SceneMessage {
            header: Header::ReplaceLayer {
                layer: Layer::Rays.name().to_string(),
            },
            payload,
        });
    }

    Ok(messages)
}

/// Maps each non-rays node to itself, keyed by uid.
fn non_rays_by_uid(scene: &Scene) -> HashMap<&str, &SceneNode> {
    scene
        .nodes()
        .iter()
        .filter(|node| node.layer != Layer::Rays)
        .map(|node| (node.uid.as_str(), node))
        .collect()
}

/// Whether a node's translation or rotation moved beyond [`EPSILON`].
fn transform_changed(old: &SceneNode, new: &SceneNode) -> bool {
    let translation = (old.transform.translation.vector - new.transform.translation.vector).norm();
    let rotation = old.transform.rotation.angle_to(&new.transform.rotation);
    translation > EPSILON || rotation > EPSILON
}

/// Builds a [`TransformEntry`] with translation relative to `origin`, as `f32`.
fn transform_entry(node: &SceneNode, origin: Point3<f64>) -> TransformEntry {
    let t = node.transform.translation.vector;
    let q = node.transform.rotation.coords;
    TransformEntry {
        uid: node.uid.clone(),
        translation: [
            to_f32(t.x - origin.x),
            to_f32(t.y - origin.y),
            to_f32(t.z - origin.z),
        ],
        rotation: [to_f32(q.x), to_f32(q.y), to_f32(q.z), to_f32(q.w)],
    }
}

/// The `rays`-layer fingerprint: each node's uid mapped to a hash of its content
/// and its transform. Comparing two scenes' maps detects any added, removed,
/// re-shaped or moved ray node.
fn rays_fingerprints(scene: &Scene) -> HashMap<&str, u64> {
    scene
        .nodes()
        .iter()
        .filter(|node| node.layer == Layer::Rays)
        .map(|node| {
            let mut hasher = DefaultHasher::new();
            hasher.write_u64(scene.node_content_hash(node));
            let t = node.transform.translation.vector;
            let q = node.transform.rotation.coords;
            for value in [t.x, t.y, t.z, q.x, q.y, q.z, q.w] {
                hasher.write_u64(value.to_bits());
            }
            (node.uid.as_str(), hasher.finish())
        })
        .collect()
}
