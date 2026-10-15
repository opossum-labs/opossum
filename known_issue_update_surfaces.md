# Known issue: a node's properties and its surfaces can disagree

A node's geometry properties are the source of truth for its shape. The traced surfaces
(`OpticSurface`s) and the body of a volume node are *built* from them by `update_surfaces()`.
Setting a property without running that step leaves the runtime surfaces describing the old shape.

Found while building the 3D view (September 2026). The 3D view no longer depends on it (October
2026, see below), and no reader outside an analysis is known to be affected any more. The root
cause is not fixed; this note exists so it is not forgotten.

## What is inconsistent

Three setters on a node write to `NodeAttr` and then rebuild the surfaces. A fourth does not:

| Setter | Rebuilds surfaces |
|---|---|
| `OpticNodeExt::set_alignment` / `clear_alignment` (`opossum_core/src/core_optics/optic_node_ext.rs`) | yes |
| `OpticNodeExt::set_coating` | yes |
| `OpticNodeExt::set_lidt` | yes |
| `NodeAttrExt::set_property` (`opossum_core/src/core_optics/node_attr_ext.rs`) | **no** |

The odd one out is the one that carries the **shape-defining** values: `center thickness` and the
curvatures. The backend's `apply_patch_property` (`opossum_backend/src/undo/node_commands.rs`), the
handler behind `PATCH /api/nodes/{uuid}/properties/{prop_name}`, calls `NodeAttr::set_property`
directly.

## Why it stays hidden

Every analysis rebuilds all surfaces before it traces (`NodeGroup::rebuild_surfaces` at the start of
the ray-trace and ghost-focus analyzers, `OpticNode::prepare_volume` for volume nodes), and
`OpmDocument::positioned_copy` reads the document back, which rebuilds on deserialization. The
stale state is therefore repaired before anything computes with it.

## How it surfaced, and how the 3D view is out of it

The 3D view serves one glTF file per component, read straight from the live document, while the
scene manifest beside it is built from a `positioned_copy`. After changing a lens thickness the
manifest reported a new geometry hash, the viewer refetched, and the component endpoint answered
with the **old** mesh.

At first `get_scene_node` (`opossum_backend/src/document.rs`) rebuilt the node's surfaces before
meshing it. That workaround is gone: the 3D view now derives every shape from the node's properties
at its current placement (`Geometry::place`), the body of a volume node through
`Volumetric::volume_body` and the surface of a planar node through `Planar::surface_mesh`. Neither
reads the runtime surfaces.

Pinned in `opossum_backend/src/scene_export.rs` by
`reshaping_a_component_changes_the_file_its_endpoint_serves` (a lens),
`bending_a_mirror_changes_the_file_its_endpoint_serves` (a mirror) and
`a_component_draws_the_same_from_the_document_as_from_a_placed_model`, and in
`opossum_core/src/core_optics/planar.rs` by `the_mesh_follows_a_changed_property_without_a_rebuild`.

## Two ways out

1. **Make `NodeAttrExt::set_property` rebuild like its siblings.** Fixes every reader at once.
   Needs thought about the blast radius: it would run on every property write, including
   non-geometric ones, and touches analysis, undo and any other caller.
2. **Keep the contract and state it.** A caller who writes a geometry property and then reads the
   runtime surfaces outside an analysis has to run `update_surfaces()` itself; everything that only
   needs the shape derives it from the properties, as the 3D view does.

Either is a decision about shared core behaviour.
