# optoscene

A Rust glTF/GLB export pipeline for the 3D visualization of optical systems,
rendered with [three.js](https://threejs.org/).

`optoscene` is an **export** crate, not a geometry crate. It turns ready-made geometry into
glTF 2.0 (GLB or `.gltf` + `.bin`) and makes that convenient for optical scenes. It does
**not** compute geometry from physical quantities: tessellation, normals and the true ray
shape are supplied by the caller, because only there are parametrization, apertures and
vignetting known. The single exception is a deliberately crude default envelope
(`Envelope::DefaultRound`) for callers that only have ray lines but still want a 3D body.

## Install

```toml
[dependencies]
optoscene = { path = "crates/optoscene" }
# Build scenes with the same nalgebra version optoscene uses:
nalgebra = { version = "0.35", default-features = false, features = ["std"] }
```

## Quick start

```rust
use nalgebra::{Isometry3, Point3};
use optoscene::{Layer, Material, Scene, SceneNode, SceneOptions, SurfacePatch, TriMesh};

// A `Scene` is the mutable container you add materials, meshes and nodes to
// before exporting. With `SceneOptions::default()` the export is centered on the
// bounding box of all geometry and no rotation is applied to the root.
let mut scene = Scene::new(SceneOptions::default());

// Register a material and get an id to reference it by. Materials are
// content-deduplicated: adding an identical one returns the id already stored.
let material = scene.add_material(Material::Opaque {
    color: [0.8, 0.2, 0.2, 1.0], // linear RGBA, every channel in [0, 1]
    metallic: 0.0,               // 0 = dielectric, 1 = metal
    roughness: 0.6,              // 0 = mirror-smooth, 1 = fully diffuse
});

// Register the geometry. A `TriMesh` is one or more `SurfacePatch`es; patches do
// not share vertices, so the edges between them stay sharp. `add_mesh` validates
// the mesh and returns an id (identical meshes are deduplicated too).
let mesh = scene.add_mesh(TriMesh {
    patches: vec![SurfacePatch {
        name: None, // optional patch label, exported to the glTF
        // Vertex positions in the mesh's own local frame, in meters.
        positions: vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(0.05, 0.0, 0.0),
            Point3::new(0.0, 0.05, 0.0),
        ],
        normals: None, // `None` -> no NORMAL attribute; viewers use flat shading
        colors: None,  // optional per-vertex linear RGBA
        indices: vec![[0, 1, 2]], // triangles as index triples into `positions`
        material,                 // the material this patch is drawn with
    }],
})?;

// Place an instance of the mesh in the world. A node ties a mesh to a transform,
// a layer and optional metadata.
scene.add_node(SceneNode {
    uid: "triangle-1".to_string(),    // stable id, unique within the scene
    name: "triangle".to_string(),     // human-readable name
    mesh: Some(mesh),                 // geometry to instance (`None` = empty node)
    transform: Isometry3::identity(), // local -> world placement (here: unmoved)
    layer: Layer::Optics,             // Optics | Rays | Aux; each is exportable on its own
    data: None,                       // optional JSON metadata, written to glTF extras
})?;

// Export the whole scene as a self-contained GLB.
scene.write_glb(std::fs::File::create("hello.glb")?)?; // or `to_glb()` for a Vec<u8>
```

Open the resulting `.glb` in the [three.js editor](https://threejs.org/editor/) or the
[Khronos glTF Validator](https://github.khronos.org/glTF-Validator/).

## Features

| Feature | Effect |
|---|---|
| *(default)* | Core export only (`nalgebra`, `serde`, `serde_json`). |
| `protocol` | Streaming frame format and the `diff` module. |
| `fixtures` | Ready-made test geometries (`cube`, `biconvex_lens`, ray bundles). |

## Documentation

- **Handbook** — the step-by-step guide in [`docs/book`](docs/book). Build it with
  `mdbook serve docs/book`, or read the markdown under [`docs/book/src`](docs/book/src).
  It covers the core concepts, building components, rays, every export form, the streaming
  protocol, and embedding in a web backend.
- **API docs** — `cargo doc -p optoscene --all-features --no-deps --open`.
- **Examples** — `export_basic` and `export_rays` (`cargo run -p optoscene --example
  export_basic --features fixtures`), and `examples/actix-viewer` for the full server + three.js
  streaming viewer (`cargo run -p actix-viewer`).

## Workspace layout

- `crates/optoscene` — the core export library. Transport-agnostic: it returns `Vec<u8>`;
  whoever embeds it ships the bytes themselves.
- `crates/optoscene-protocol` — the frame format for streaming scene updates. Compiles for
  `wasm32-unknown-unknown` (no `std::fs`, no threads).
- `examples/actix-viewer` — an example actix-web server plus a three.js viewer, showing the
  embedding pattern end to end.

## License

GPL-3.0.
