//! Writes `target/out/basic.glb`: one object per material type, plus a biconvex
//! lens instanced twice.
//!
//! Run with `cargo run -p optoscene --example export_basic --features fixtures`.

use std::path::Path;

use nalgebra::Isometry3;
use optoscene::{fixtures, Layer, Material, MeshId, Scene, SceneNode, SceneOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scene = Scene::new(SceneOptions {
        name: "basic".to_string(),
        ..SceneOptions::default()
    });

    let glass = scene.add_material(Material::Glass {
        color: [0.9, 0.95, 1.0],
        ior: 1.5,
        thickness: 0.01,
    });
    let mirror = scene.add_material(Material::Mirror {
        color: [0.9, 0.9, 0.9],
        roughness: 0.05,
    });
    let opaque = scene.add_material(Material::Opaque {
        color: [0.8, 0.3, 0.2, 1.0],
        metallic: 0.1,
        roughness: 0.6,
    });
    let unlit = scene.add_material(Material::Unlit {
        color: [1.0, 0.9, 0.2, 1.0],
    });
    let translucent = scene.add_material(Material::Translucent {
        color: [0.2, 0.6, 1.0, 0.3],
    });

    let lens = scene.add_mesh(fixtures::biconvex_lens(0.05, 0.01, 0.1, 0.1, 48, glass))?;
    let mirror_cube = scene.add_mesh(fixtures::cube(0.02, mirror))?;
    let opaque_cube = scene.add_mesh(fixtures::cube(0.02, opaque))?;
    let unlit_cube = scene.add_mesh(fixtures::cube(0.02, unlit))?;
    let translucent_cube = scene.add_mesh(fixtures::cube(0.02, translucent))?;

    // The lens, instanced twice.
    place(&mut scene, "lens-1", lens, 0.00)?;
    place(&mut scene, "lens-2", lens, 0.05)?;
    // One object per remaining material type.
    place(&mut scene, "mirror", mirror_cube, 0.12)?;
    place(&mut scene, "opaque", opaque_cube, 0.17)?;
    place(&mut scene, "unlit", unlit_cube, 0.22)?;
    place(&mut scene, "translucent", translucent_cube, 0.27)?;

    let out_dir = Path::new("target/out");
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join("basic.glb");
    scene.write_glb(std::fs::File::create(&path)?)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Places a node referencing `mesh` at world position `(x, 0, 0)`.
fn place(scene: &mut Scene, uid: &str, mesh: MeshId, x: f64) -> Result<(), optoscene::Error> {
    scene.add_node(SceneNode {
        uid: uid.to_string(),
        name: uid.to_string(),
        mesh: Some(mesh),
        transform: Isometry3::translation(x, 0.0, 0.0),
        layer: Layer::Optics,
        data: None,
    })
}
