//! Writes `target/out/rays.glb`: a lens plus a focused ray bundle, shown once
//! with the default round envelope and once with a hand-built mesh envelope.
//!
//! Run with `cargo run -p optoscene --example export_rays --features fixtures`.

use std::path::Path;

use nalgebra::{Isometry3, Point3};
use optoscene::{
    fixtures, Envelope, Layer, Material, RayStyle, Scene, SceneNode, SceneOptions, SurfacePatch,
    TriMesh,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scene = Scene::new(SceneOptions {
        name: "rays".to_string(),
        ..SceneOptions::default()
    });

    let glass = scene.add_material(Material::Glass {
        color: [0.9, 0.95, 1.0],
        ior: 1.5,
        thickness: 0.01,
    });
    let lens = scene.add_mesh(fixtures::biconvex_lens(0.05, 0.01, 0.1, 0.1, 48, glass))?;
    scene.add_node(SceneNode {
        uid: "lens".to_string(),
        name: "lens".to_string(),
        mesh: Some(lens),
        transform: Isometry3::identity(),
        layer: Layer::Optics,
        data: None,
    })?;

    // Focused bundle with the crude default round envelope.
    let mut default_beam = fixtures::focusing_bundle(0.005);
    default_beam.uid = "beam-default".to_string();
    default_beam.wavelength = Some(532e-9);
    scene.add_ray_trace(
        &default_beam,
        &RayStyle {
            max_lines: 40,
            envelope: Envelope::default_round(),
            ..RayStyle::default()
        },
    )?;

    // A second bundle, offset in x, with a hand-built mesh envelope.
    let mut mesh_beam = fixtures::focusing_bundle(0.005);
    mesh_beam.uid = "beam-mesh".to_string();
    mesh_beam.wavelength = Some(633e-9);
    for station in &mut mesh_beam.stations {
        for point in station.iter_mut().flatten() {
            point.x += 0.08;
        }
    }
    let envelope_material = scene.add_material(Material::Translucent {
        color: [1.0, 0.3, 0.3, 0.25],
    });
    let envelope = square_tube(0.08, 0.006, 0.0, 0.1, envelope_material);
    scene.add_ray_trace(
        &mesh_beam,
        &RayStyle {
            max_lines: 40,
            envelope: Envelope::Mesh(envelope),
            ..RayStyle::default()
        },
    )?;

    let out_dir = Path::new("target/out");
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join("rays.glb");
    scene.write_glb(std::fs::File::create(&path)?)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// A hand-built square tube (four side walls, no caps) around `(cx, 0, z)`.
fn square_tube(cx: f64, r: f64, z0: f64, z1: f64, material: optoscene::MaterialId) -> TriMesh {
    let ring = |z: f64| {
        [
            Point3::new(cx + r, 0.0, z),
            Point3::new(cx, r, z),
            Point3::new(cx - r, 0.0, z),
            Point3::new(cx, -r, z),
        ]
    };
    let mut positions = Vec::new();
    positions.extend_from_slice(&ring(z0));
    positions.extend_from_slice(&ring(z1));
    let mut indices = Vec::new();
    for i in 0..4u32 {
        let next = (i + 1) % 4;
        indices.push([i, next, 4 + next]);
        indices.push([i, 4 + next, 4 + i]);
    }
    TriMesh {
        patches: vec![SurfacePatch {
            name: Some("beam-envelope".to_string()),
            positions,
            normals: None,
            colors: None,
            indices,
            material,
        }],
    }
}
