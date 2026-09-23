//! Pure diff function: computes the minimal op list to transition the JS renderer from a
//! known state (`applied`) to a new state (`next`).

use crate::op::{Op, WireSource};
use crate::{GlbObject, GlbSource};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Computes the minimal op list and updates `applied` in-place.
///
/// # Order
///
/// Removes first, then updates (transform/visible/selected), then adds. This ensures that
/// during a full scene swap, at no point are `|old| + |new|` models simultaneously on the
/// GPU.
///
/// # Duplicates
///
/// Multiple entries with the same `id` in `next` are resolved "first-wins"; the extra IDs
/// are collected in `duplicates` so `viewer.rs` can report them as `ViewerEvent::Error`.
///
/// # Camera
///
/// None of the ops produced by this function touch the camera. `FitView` and `ResetCamera`
/// are intentionally not producible here; they exist exclusively in `ViewerHandle`.
///
/// # Arguments
///
/// * `applied`    - State the renderer already knows. Updated in-place.
/// * `next`       - The caller's new desired state.
/// * `duplicates` - Populated with duplicate IDs (for error reporting to the caller).
///
/// # Returns
///
/// The minimal op list. May be empty if `applied` and `next` are identical.
pub fn diff(
    applied: &mut BTreeMap<String, GlbObject>,
    next: &[GlbObject],
    duplicates: &mut Vec<String>,
) -> Vec<Op> {
    // Dedup: first-wins.
    // NOTE: BTreeMap::insert returns the *old* value and stores the *new* one — that would
    // let the *last* entry win. We check with contains_key first instead.
    let mut seen: BTreeMap<&str, &GlbObject> = BTreeMap::new();
    for obj in next {
        if seen.contains_key(obj.id.as_str()) {
            duplicates.push(obj.id.clone());
        } else {
            seen.insert(obj.id.as_str(), obj);
        }
    }

    let mut ops = Vec::new();

    // 1. Removes: objects that no longer appear in the new state.
    let gone: Vec<String> = applied
        .keys()
        .filter(|id| !seen.contains_key(id.as_str()))
        .cloned()
        .collect();
    for id in gone {
        applied.remove(&id);
        ops.push(Op::Remove { id });
    }

    // 2. Updates and 3. Adds.
    for (id, want) in &seen {
        match applied.get(*id) {
            None => {
                // New: Add
                ops.push(Op::Add {
                    id: (*id).to_owned(),
                    source: to_wire_source(&want.source, id),
                    transform: want.transform,
                    visible: want.visible,
                    selected: want.selected,
                });
                applied.insert((*id).to_owned(), (*want).clone());
            }
            Some(have) if have.source != want.source => {
                // Source changed → Reload (disposes the old model and reloads).
                // Transform/visible/selected travel in the same op so the new model
                // never appears for a frame in the wrong pose.
                ops.push(Op::Reload {
                    id: (*id).to_owned(),
                    source: to_wire_source(&want.source, id),
                    transform: want.transform,
                    visible: want.visible,
                    selected: want.selected,
                });
                applied.insert((*id).to_owned(), (*want).clone());
            }
            Some(have) => {
                // Same source — send only the changed fields.
                let prev_len = ops.len();
                if have.transform != want.transform {
                    ops.push(Op::SetTransform {
                        id: (*id).to_owned(),
                        transform: want.transform,
                    });
                }
                if have.visible != want.visible {
                    ops.push(Op::SetVisible {
                        id: (*id).to_owned(),
                        visible: want.visible,
                    });
                }
                if have.selected != want.selected {
                    ops.push(Op::SetSelected {
                        id: (*id).to_owned(),
                        selected: want.selected,
                    });
                }
                if ops.len() != prev_len {
                    applied.insert((*id).to_owned(), (*want).clone());
                }
            }
        }
    }

    ops
}

/// Translates a [`GlbSource`] into a [`WireSource`] for transport.
///
/// `Bytes` sources receive a transport key derived from the ID plus a clone of the payload
/// `Arc`. The payload travels with the op, so `wire::messages` streams exactly the bytes
/// `diff` picked (first-wins) — there is no separate lookup that could miss or disagree.
///
/// # Arguments
///
/// * `source` - The source specification.
/// * `id`     - The object ID (used as the key base for byte streams).
///
/// # Returns
///
/// The wire-ready source specification.
pub fn to_wire_source(source: &GlbSource, id: &str) -> WireSource {
    match source {
        GlbSource::Url(url) => WireSource::Url { url: url.clone() },
        GlbSource::Bytes(data) => WireSource::Bytes {
            key: id.to_owned(),
            data: Arc::clone(data),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Transform;
    use std::sync::Arc;

    fn url(u: &str) -> GlbSource {
        GlbSource::Url(u.into())
    }
    fn obj(id: &str, src: GlbSource) -> GlbObject {
        GlbObject {
            id: id.into(),
            source: src,
            transform: Transform::default(),
            visible: true,
            selected: false,
        }
    }

    // ── Base cases ───────────────────────────────────────────────────────────

    #[test]
    fn empty_to_empty_no_ops() {
        let mut applied = BTreeMap::new();
        let mut dupes = Vec::new();
        let ops = diff(&mut applied, &[], &mut dupes);
        assert!(ops.is_empty());
        assert!(applied.is_empty());
    }

    #[test]
    fn add_single_object() {
        let mut applied = BTreeMap::new();
        let mut dupes = Vec::new();
        let next = vec![obj("a", url("http://x/a.glb"))];
        let ops = diff(&mut applied, &next, &mut dupes);
        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0], Op::Add { id, .. } if id == "a"));
        assert!(applied.contains_key("a"));
    }

    #[test]
    fn remove_single_object() {
        let mut applied: BTreeMap<String, GlbObject> =
            [("a".to_owned(), obj("a", url("http://x/a.glb")))].into();
        let mut dupes = Vec::new();
        let ops = diff(&mut applied, &[], &mut dupes);
        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0], Op::Remove { id } if id == "a"));
        assert!(applied.is_empty());
    }

    #[test]
    fn no_change_no_ops() {
        let o = obj("a", url("http://x/a.glb"));
        let mut applied: BTreeMap<String, GlbObject> = [("a".to_owned(), o.clone())].into();
        let mut dupes = Vec::new();
        let ops = diff(&mut applied, &[o], &mut dupes);
        assert!(ops.is_empty());
    }

    // ── Field updates ────────────────────────────────────────────────────────

    #[test]
    fn transform_only_change() {
        let o = obj("a", url("http://x/a.glb"));
        let mut applied: BTreeMap<String, GlbObject> = [("a".to_owned(), o.clone())].into();
        let mut changed = o;
        changed.transform.position = [1.0, 0.0, 0.0];
        let mut dupes = Vec::new();
        let ops = diff(&mut applied, &[changed], &mut dupes);
        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0], Op::SetTransform { id, .. } if id == "a"));
    }

    #[test]
    fn visible_only_change() {
        let o = obj("a", url("http://x/a.glb"));
        let mut applied: BTreeMap<String, GlbObject> = [("a".to_owned(), o.clone())].into();
        let mut changed = o;
        changed.visible = false;
        let mut dupes = Vec::new();
        let ops = diff(&mut applied, &[changed], &mut dupes);
        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0], Op::SetVisible { id, visible: false } if id == "a"));
    }

    #[test]
    fn selected_only_change() {
        let o = obj("a", url("http://x/a.glb"));
        let mut applied: BTreeMap<String, GlbObject> = [("a".to_owned(), o.clone())].into();
        let mut changed = o;
        changed.selected = true;
        let mut dupes = Vec::new();
        let ops = diff(&mut applied, &[changed], &mut dupes);
        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0], Op::SetSelected { id, selected: true } if id == "a"));
    }

    #[test]
    fn source_change_emits_reload() {
        let o = obj("a", url("http://x/a.glb"));
        let mut applied: BTreeMap<String, GlbObject> = [("a".to_owned(), o)].into();
        let changed = obj("a", url("http://x/b.glb"));
        let mut dupes = Vec::new();
        let ops = diff(&mut applied, &[changed], &mut dupes);
        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0], Op::Reload { id, .. } if id == "a"));
    }

    #[test]
    fn bytes_source_change_via_new_arc() {
        let arc1: Arc<[u8]> = Arc::from(b"bytes".as_slice());
        let arc2: Arc<[u8]> = Arc::from(b"bytes".as_slice()); // new allocation → unequal
        let o = obj("a", GlbSource::Bytes(arc1));
        let mut applied: BTreeMap<String, GlbObject> = [("a".to_owned(), o)].into();
        let changed = obj("a", GlbSource::Bytes(arc2));
        let mut dupes = Vec::new();
        let ops = diff(&mut applied, &[changed], &mut dupes);
        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0], Op::Reload { id, .. } if id == "a"));
    }

    // ── Ordering ─────────────────────────────────────────────────────────────

    #[test]
    fn reorder_produces_no_ops() {
        let a = obj("a", url("http://x/a.glb"));
        let b = obj("b", url("http://x/b.glb"));
        let mut applied: BTreeMap<String, GlbObject> =
            [("a".to_owned(), a.clone()), ("b".to_owned(), b.clone())].into();
        let mut dupes = Vec::new();
        // b first, then a
        let ops = diff(&mut applied, &[b, a], &mut dupes);
        assert!(ops.is_empty(), "reordering must not produce ops: {ops:?}");
    }

    #[test]
    fn removes_come_before_adds() {
        // Old scene: {a}  New scene: {b}  → Remove(a) before Add(b)
        let a = obj("a", url("http://x/a.glb"));
        let b = obj("b", url("http://x/b.glb"));
        let mut applied: BTreeMap<String, GlbObject> = [("a".to_owned(), a)].into();
        let mut dupes = Vec::new();
        let ops = diff(&mut applied, &[b], &mut dupes);
        assert_eq!(ops.len(), 2);
        assert!(
            matches!(&ops[0], Op::Remove { id } if id == "a"),
            "first op must be Remove"
        );
        assert!(
            matches!(&ops[1], Op::Add { id, .. } if id == "b"),
            "second op must be Add"
        );
    }

    // ── Duplicates ───────────────────────────────────────────────────────────

    #[test]
    fn duplicate_id_first_wins_and_reported() {
        let a1 = obj("a", url("http://x/a1.glb"));
        let a2 = obj("a", url("http://x/a2.glb"));
        let mut applied = BTreeMap::new();
        let mut dupes = Vec::new();
        let ops = diff(&mut applied, &[a1, a2], &mut dupes);
        // Only one Add, using the first source
        assert_eq!(ops.len(), 1);
        assert!(
            matches!(&ops[0], Op::Add { source: WireSource::Url { url }, .. } if url == "http://x/a1.glb")
        );
        assert_eq!(dupes, vec!["a"]);
    }

    // ── Multiple simultaneous changes ─────────────────────────────────────────

    #[test]
    fn multiple_field_changes_emit_multiple_ops() {
        let o = obj("a", url("http://x/a.glb"));
        let mut applied: BTreeMap<String, GlbObject> = [("a".to_owned(), o.clone())].into();
        let mut changed = o;
        changed.transform.position = [1.0, 0.0, 0.0];
        changed.visible = false;
        let mut dupes = Vec::new();
        let ops = diff(&mut applied, &[changed], &mut dupes);
        assert_eq!(ops.len(), 2);
    }
}
