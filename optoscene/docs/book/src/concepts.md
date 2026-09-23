# Core concepts

Everything is assembled into a [`Scene`], which you then export. This chapter walks through
the pieces you put into it.

## The `Scene`

```rust
let mut scene = Scene::new(SceneOptions::default());
```

A `Scene` is a mutable container for materials, meshes and placed nodes. You add things to
it, then call one of the export methods ([Exporting](./exporting.md)). Adding an identical
material or mesh twice returns the id already registered, and the export always follows
insertion order, so the output is deterministic.

`SceneOptions` controls three things:

| Field | Meaning | Default |
|---|---|---|
| `name` | The glTF scene name. | `"optoscene"` |
| `origin` | Reference point subtracted from all world positions before the `f32` cast. `None` = center of the world-space bounding box of all nodes. | `None` |
| `root_rotation` | Rotation applied to the root node (a `UnitQuaternion`). | identity |

## Ids and deduplication

Registering a material or mesh returns a small copyable handle:

```rust
let glass: MaterialId = scene.add_material(Material::Glass { /* … */ });
let lens: MeshId = scene.add_mesh(my_trimesh)?;
```

Materials and meshes are **content-deduplicated**: add the exact same value twice and you get
the same id back, one glTF material/mesh, and two nodes that reference it (instancing). The
comparison hashes the content (floats by their bit pattern), so it is exact.

## Materials

`Material` is a small set of presets, each mapping to a glTF PBR material (plus extensions):

| `Material` | glTF result |
|---|---|
| `Glass { color, ior, thickness }` | metallic 0, roughness 0.05, `KHR_materials_transmission` (1), `KHR_materials_ior`, `KHR_materials_volume` (thickness). |
| `Mirror { color, roughness }` | metallic 1, roughness as given. |
| `Opaque { color, metallic, roughness }` | the values as given. |
| `Unlit { color }` | `KHR_materials_unlit`; `alphaMode = BLEND` when the alpha < 1. |
| `Translucent { color }` | metallic 0, roughness 0.5, `alphaMode = BLEND`, double-sided. Used for bundle envelopes. |

Colors are **linear** RGB/RGBA (glTF factors are linear). `Glass`/`Mirror` take `[f32; 3]`;
`Opaque`/`Unlit`/`Translucent` take `[f32; 4]` with alpha.

Only the extensions actually used are listed in `extensionsUsed`, and `extensionsRequired` is
never written — so viewers that do not understand an extension still load the file with a
fallback.

## Meshes: patches and `TriMesh`

A mesh is a `TriMesh`, which is a list of `SurfacePatch`es:

```rust
pub struct SurfacePatch {
    pub name: Option<String>,
    pub positions: Vec<Point3<f64>>,       // meters, in the mesh's local frame
    pub normals: Option<Vec<Vector3<f64>>>, // optional; see below
    pub colors: Option<Vec<[f32; 4]>>,      // optional linear RGBA vertex colors
    pub indices: Vec<[u32; 3]>,             // triangles, into this patch's positions
    pub material: MaterialId,
}
```

Two things are worth internalizing:

- **Patches do not share vertices.** Each patch owns its own positions and indices, so the
  seam between two patches stays a hard edge (exactly what you want between a lens surface and
  its cylindrical rim). One glTF *primitive* is emitted per patch.
- **Normals are never generated for you.** If you supply `normals`, they are normalized to
  unit length and written as the `NORMAL` attribute. If you supply `None`, no `NORMAL`
  attribute is written and viewers fall back to flat shading. This is deliberate: the caller
  can compute exact analytic normals (`∂f/∂u × ∂f/∂v`) that no re-derivation from triangles
  could match. A supplied normal of length ≈ 0 is an error at export time.

Index width is chosen automatically: `UNSIGNED_SHORT` when a patch has ≤ 65 535 vertices,
`UNSIGNED_INT` otherwise.

## Nodes, layers and metadata

A `SceneNode` places one mesh instance in the world:

```rust
pub struct SceneNode {
    pub uid: String,                 // unique within the scene (e.g. an Opossum UUID)
    pub name: String,                // human-readable
    pub mesh: Option<MeshId>,
    pub transform: Isometry3<f64>,   // local mesh frame → world
    pub layer: Layer,                // Optics | Rays | Aux
    pub data: Option<serde_json::Value>, // arbitrary metadata
}
```

- `uid` must be unique; a duplicate is `Error::DuplicateUid`. It is also the key the streaming
  protocol uses to add, move and remove nodes.
- `transform` is an isometry (rotation + translation), no scale.
- `layer` groups the node. There are three: `Optics` (components), `Rays` (ray lines and
  envelopes) and `Aux` (helpers). Layers can be exported or replaced individually — the
  streaming diff replaces the whole `Rays` layer at once, for instance.
- `data` is arbitrary JSON. It is written to the node's glTF `extras`, which three.js exposes
  as `object.userData` — a convenient channel for focal lengths, part numbers, anything.

In the exported glTF, `uid`, `layer` and (if present) `data` all land in the node's `extras`.

## The origin

Optical systems can sit far from the coordinate origin (meters down a beamline), and `f32`
loses precision far from zero. So before the `f64 → f32` cast, `optoscene` subtracts a single
**origin** from every world position and stores it once on the root node.

- If `SceneOptions::origin` is `Some(p)`, that point is used.
- If it is `None`, the origin is the center of the axis-aligned bounding box of *all* node
  vertices — computed by [`Scene::effective_origin`], which is public because the streaming
  diff needs it.

Exported node translations are therefore `world − origin`, and the root node carries
`extras.origin = [x, y, z]` (in `f64` meters) so a consumer can recover absolute coordinates.

## Validation and errors

`add_mesh` validates before registering: at least one patch, each patch with at least one
triangle, all indices in range, `normals`/`colors` (if present) the same length as
`positions`, all values finite, and every referenced material registered. Anything wrong
returns an [`Error`](./reference.md#errors) — never a panic. The crate does not otherwise touch
your geometry: positions, indices and normals pass through unchanged apart from the origin
shift and the `f32` cast.

Next: [Building a lens](./building-a-lens.md).

[`Scene`]: ./reference.md
[`Scene::effective_origin`]: ./exporting.md
