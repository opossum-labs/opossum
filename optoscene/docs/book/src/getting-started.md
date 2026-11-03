# Getting started

## Add the crate

`optoscene` is part of the OPOSSUM repository. Depend on it by path (or git):

```toml
[dependencies]
optoscene = { path = "../optoscene/crates/optoscene" }
# You also build scenes with nalgebra types, so depend on the same version:
nalgebra = { version = "0.35", default-features = false, features = ["std"] }
```

> The `nalgebra` version **must** match the one `optoscene` uses, otherwise the
> `Point3`/`Isometry3` types are incompatible across the crate boundary.

### Feature flags

| Feature | Effect |
|---|---|
| *(default)* | Core export only. Pulls in `nalgebra`, `serde`, `serde_json` and nothing else. |
| `protocol` | Enables the streaming frame format and the `diff` module (see [Streaming updates](./streaming.md)). |
| `fixtures` | Exposes the ready-made test geometries (`cube`, `biconvex_lens`, ray bundles) to your own code and examples. |

```toml
optoscene = { path = "...", features = ["protocol", "fixtures"] }
```

## Your first GLB

The smallest useful program: create a scene, register a material, add a mesh, place a node,
and write a `.glb` file. This mirrors the `export_basic` example.

```rust
use std::path::Path;

use nalgebra::{Isometry3, Point3};
use optoscene::{
    Layer, Material, Scene, SceneNode, SceneOptions, SurfacePatch, TriMesh,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scene = Scene::new(SceneOptions {
        name: "hello".to_string(),
        ..SceneOptions::default()
    });

    // 1. Register a material; identical materials are deduplicated.
    let material = scene.add_material(Material::Opaque {
        color: [0.8, 0.2, 0.2, 1.0], // linear RGBA
        metallic: 0.0,
        roughness: 0.6,
    });

    // 2. Register a mesh. Here: one triangle (positions in meters).
    let mesh = scene.add_mesh(TriMesh {
        patches: vec![SurfacePatch {
            name: None,
            positions: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.05, 0.0, 0.0),
                Point3::new(0.0, 0.05, 0.0),
            ],
            normals: None, // not generated for you; omit for flat shading
            colors: None,
            indices: vec![[0, 1, 2]],
            material,
        }],
    })?;

    // 3. Place an instance of the mesh in the world.
    scene.add_node(SceneNode {
        uid: "triangle-1".to_string(),
        name: "triangle".to_string(),
        mesh: Some(mesh),
        transform: Isometry3::identity(),
        layer: Layer::Optics,
        data: None,
    })?;

    // 4. Export.
    std::fs::create_dir_all("target/out")?;
    scene.write_glb(std::fs::File::create(Path::new("target/out/hello.glb"))?)?;
    Ok(())
}
```

`write_glb` streams the bytes to any `std::io::Write`. If you want the bytes in memory
instead, use `to_glb()` which returns a `Vec<u8>`.

## Run the bundled example

The crate ships a richer example that builds a lens and one object per material type:

```bash
cargo run -p optoscene --example export_basic --features fixtures
# writes target/out/basic.glb
```

## View the result

A `.glb` file is a standard glTF binary; open it in any glTF viewer:

- The **three.js editor** (<https://threejs.org/editor/>) — drag the `.glb` in.
- The **Khronos glTF Validator** (<https://github.khronos.org/glTF-Validator/>) — confirms
  the file has zero errors.
- Windows **3D Viewer**, Blender, or VS Code's glTF extensions.

Glass materials use transmission and look dark without an environment map; the three.js
editor supplies one. See [Embedding in a web backend](./web-backend.md#renderer-notes).

Next: [Core concepts](./concepts.md).
