# Reference

A compact index of the public API. The authoritative, always-current version is the API doc:

```bash
cargo doc -p optoscene --all-features --no-deps --open
```

## Types

| Type | Summary |
|---|---|
| `Scene` | The mutable container; build it, then export. |
| `SceneOptions` | `name`, `origin: Option<Point3<f64>>`, `root_rotation: UnitQuaternion<f64>`. |
| `MaterialId`, `MeshId` | Copyable handles returned by `add_material` / `add_mesh`. |
| `Material` | `Glass`, `Mirror`, `Opaque`, `Unlit`, `Translucent`. |
| `SurfacePatch` | `name`, `positions`, `normals?`, `colors?`, `indices`, `material`. |
| `TriMesh` | `patches: Vec<SurfacePatch>`. |
| `SceneNode` | `uid`, `name`, `mesh?`, `transform`, `layer`, `data?`. |
| `Layer` | `Optics`, `Rays`, `Aux`; `.name()` → `"optics"`/`"rays"`/`"aux"`. |
| `RayTrace` | `uid`, `stations`, `wavelength?`. |
| `RayStyle` | `max_lines`, `line_color?`, `envelope`, `envelope_color`. |
| `Envelope` | `None`, `Mesh(TriMesh)`, `DefaultRound { sectors, steps }`; `::default_round()`. |

With the `protocol` feature also: `SceneMessage`, `Header`, `TransformEntry`, and the free
functions `full_scene` and `diff`.

## `Scene` methods

```rust
fn new(options: SceneOptions) -> Self;
fn add_material(&mut self, material: Material) -> MaterialId;
fn add_mesh(&mut self, mesh: TriMesh) -> Result<MeshId, Error>;
fn add_node(&mut self, node: SceneNode) -> Result<(), Error>;
fn add_ray_trace(&mut self, trace: &RayTrace, style: &RayStyle) -> Result<(), Error>;

fn to_glb(&self) -> Result<Vec<u8>, Error>;
fn write_glb<W: std::io::Write>(&self, writer: W) -> Result<(), Error>;
fn to_gltf_separate(&self, bin_uri: &str) -> Result<(String, Vec<u8>), Error>;
fn write_gltf(&self, dir: &std::path::Path, stem: &str) -> Result<(), Error>;
fn layer_to_glb(&self, layer: Layer) -> Result<Vec<u8>, Error>;
fn nodes_to_glb(&self, uids: &[&str]) -> Result<Vec<u8>, Error>;
fn effective_origin(&self) -> Point3<f64>;
```

## Feature flags

| Feature | Effect |
|---|---|
| *(default)* | Core export. Deps: `nalgebra`, `serde`, `serde_json`. |
| `protocol` | Streaming frame format + `diff` module (adds the `optoscene-protocol` dep). |
| `fixtures` | Test geometries (`cube`, `biconvex_lens`, ray bundles) for your code and examples. |

## Material mapping

| `Material` | glTF |
|---|---|
| `Glass { color, ior, thickness }` | metallic 0, roughness 0.05; `KHR_materials_transmission {1}`, `KHR_materials_ior {ior}`, `KHR_materials_volume {thickness}`. |
| `Mirror { color, roughness }` | metallic 1, roughness as given. |
| `Opaque { color, metallic, roughness }` | as given. |
| `Unlit { color }` | `KHR_materials_unlit`; `alphaMode BLEND` if alpha < 1. |
| `Translucent { color }` | metallic 0, roughness 0.5, `alphaMode BLEND`, double-sided. |

## Errors

The crate's error type. Library code returns it instead of panicking on bad input.

| Variant | Cause |
|---|---|
| `InvalidMesh(String)` | Empty mesh/patch, out-of-range index, mismatched normals/colors length, non-finite or zero-length value. |
| `InvalidRayTrace(String)` | Fewer than two stations, unequal station lengths, non-finite coordinate. |
| `UnknownMaterial(MaterialId)` | A patch references an unregistered material. |
| `UnknownMesh(MeshId)` | A node references an unregistered mesh. |
| `DuplicateUid(String)` | Two nodes share a uid. |
| `UnknownUid(String)` | A uid was expected but not present. |
| `Io(std::io::Error)` | Writing output failed. |
| `Json(serde_json::Error)` | Serialization failed. |
| `Frame(FrameError)` | *(`protocol` feature)* Encoding a message into a frame failed. |

`Error` implements `Display`, `std::error::Error`, and `From` for `io::Error` /
`serde_json::Error` (and `FrameError` under `protocol`).

## Streaming (`protocol`)

```rust
fn full_scene(scene: &Scene) -> Result<SceneMessage, Error>;
fn diff(old: &Scene, new: &Scene) -> Result<Vec<SceneMessage>, Error>;
impl SceneMessage { fn to_frame(&self) -> Result<Vec<u8>, Error>; }
```

Frame layout, header variants and the diff rules are in [Streaming updates](./streaming.md).
`optoscene-protocol` provides `encode`, `decode`, `Header`, `TransformEntry` and `FrameError`.

## Glossary

- **Station** — a surface at which ray positions are sampled; rays run straight between
  stations. `None` at a station marks a lost ray.
- **Patch** — one smooth surface of a mesh, with its own vertices (so seams stay sharp).
- **Envelope** — the hull surface around a ray bundle: caller-provided `Mesh` (accurate) or the
  crude `DefaultRound` loft.
- **Origin** — the reference point subtracted from world positions before the `f32` cast, so
  coordinates far down a beamline stay precise.
- **Layer** — `Optics` / `Rays` / `Aux`; a group that can be exported or replaced on its own.
- **Frame** — one wire message: `OSCN` magic, version, JSON header, optional GLB payload.
