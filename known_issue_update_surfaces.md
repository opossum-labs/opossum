# Known issue: a node's properties and its surfaces can disagree

A node's geometry properties are the source of truth for its shape. The `GeoSurface`s that
everything geometric actually reads are *derived* from them by `update_surfaces()`. Setting a
property without running that step leaves the node describing one shape and behaving like another.

Found while building the 3D view (September 2026). Not fixed — the 3D view works around it. This
note exists so the underlying problem is not forgotten.

## What is inconsistent

Four setters on a node write to `NodeAttr` and then refresh the surfaces. A fifth does not:

| Setter | Refreshes surfaces |
|---|---|
| `OpticNodeExt::set_alignment` (`opossum_core/src/core_optics/optic_node_ext.rs:250`) | yes |
| `OpticNodeExt::set_aperture` (`…/optic_node_ext.rs:265`) | yes |
| `OpticNodeExt::set_coating` (`…/optic_node_ext.rs:277`) | yes |
| `OpticNodeExt::set_lidt` (`…/optic_node_ext.rs:289`) | yes |
| `NodeAttrExt::set_property` (`…/node_attr_ext.rs:139`) | **no** |

`NodeAttrExt` has a blanket impl for every node (`node_attr_ext.rs:89`), so all five are
node-level setters of equal standing. The odd one out is the one that carries the **shape-defining**
values: an aperture or a coating does not change a component's form, but `center thickness` and the
curvatures do.

`Lens::update_surfaces()` (`opossum_core/src/nodes/lens/mod.rs:232`) reads exactly
`"front curvature"`, `"rear curvature"` and `"center thickness"` out of the property map and rebuilds
the surfaces from them. `Volumetric::volume_body()` then reads those surfaces — never the properties.

The documentation states the opposite of what the code does.
`opossum_core/src/core_optics/volumetric.rs:84`:

> Changing the node's placement or any of its geometry properties runs `update_surfaces()`, which
> installs fresh surfaces, so the body has to be derived again afterwards.

True for the placement, not for the geometry properties.

The backend goes one level lower still: `apply_patch_property`
(`opossum_backend/src/undo/node_commands.rs:300`) — the handler behind `PATCH /api/nodes/{uuid}/properties/{prop_name}`
— calls `NodeAttr::set_property` directly. Using the node-level setter instead would not have helped.

## Why it stays hidden

Every analysis and every positioning run goes through `OpmDocument::positioned_copy`
(`opossum_core/src/opm_document/analysis.rs:131`), which builds its copy by serializing the document
and reading it back. Deserialization runs `after_deserialization_hook` → `update_surfaces()`
(`opossum_core/src/core_optics/optic_node.rs:127`). The stale state is therefore repaired silently
before anything computes with it.

It only becomes observable where geometry is read **without** such a run.

## How it surfaced

The 3D view serves one glTF file per component. That endpoint originally read the node straight from
the live document, because a component's shape does not depend on where it ended up. The scene
manifest beside it is built from a `positioned_copy`.

So after changing a lens thickness: the manifest reported a new geometry hash (fresh surfaces), the
viewer dutifully refetched, and the component endpoint answered with the **old** mesh (stale
surfaces). Changing the thickness appeared to do nothing at all — no error anywhere.

Pinned by `reshaping_a_component_changes_the_file_its_endpoint_serves` in
`opossum_backend/src/scene_export.rs`.

## The workaround in place

`get_scene_node` (`opossum_backend/src/document.rs`) now reads from the same `positioned_copy` the
manifest uses. The placement is still discarded — the point is only that both endpoints see the same
document state, so the hash and the bytes cannot disagree.

Its cost: one full document round-trip per component fetch. One on a shape change, but *N* when a
model with *N* components is first opened. If that becomes noticeable, either cache the placed copy
per document revision in `AppState`, or fix the root cause below and let the endpoint be cheap again.

## Two ways out

1. **Make `NodeAttrExt::set_property` refresh like its four siblings.** Matches the documented
   contract and fixes every reader at once. Needs thought about the blast radius: it would run on
   every property write, including non-geometric ones, and touches analysis, undo and any other
   caller.
2. **Change the contract instead.** If the omission is deliberate — properties also carry
   non-geometric data, and refreshing on each write may be wasteful — then correct
   `volumetric.rs:84` and state plainly that a caller who writes a geometry property must run
   `update_surfaces()` itself. Then give the callers that do so a single place to go through.

Either is a decision about shared core behaviour, which is why the 3D view took the local route.
