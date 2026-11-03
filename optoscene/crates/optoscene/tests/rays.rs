//! Integration tests for ray visualization: decimated ray lines, the default
//! round envelope and the caller-supplied mesh envelope.
//!
//! Requires the `fixtures` feature (see the crate's `[[test]]` entry), which
//! provides the ray-bundle geometries exercised here.

use gltf::mesh::Mode;
use nalgebra::Point3;
use optoscene::{
    fixtures, Envelope, Material, RayStyle, RayTrace, Scene, SceneOptions, SurfacePatch, TriMesh,
};

/// The outer bundle radius used across the acceptance tests (5 mm).
const R0: f64 = 0.005;
/// Cross-section rings per segment for the default round envelope.
const STEPS: u32 = 16;
/// Points per cross-section ring for the default round envelope.
const SECTORS: u32 = 32;

/// A scene with the origin pinned to zero.
///
/// With the origin at the coordinate origin, an exported ray node's translation
/// equals the trace's AABB center, so a world position is simply
/// `node_translation + local_position`.
fn zero_origin_scene() -> Scene {
    Scene::new(SceneOptions {
        origin: Some(Point3::origin()),
        ..SceneOptions::default()
    })
}

/// Adds a ray trace with an otherwise-default style overriding only lines and
/// envelope.
fn add_trace(scene: &mut Scene, trace: &RayTrace, max_lines: usize, envelope: Envelope) {
    scene
        .add_ray_trace(
            trace,
            &RayStyle {
                max_lines,
                envelope,
                ..RayStyle::default()
            },
        )
        .expect("ray trace added");
}

/// Whether the exported GLB contains a node with the given name.
fn has_node(glb: &[u8], name: &str) -> bool {
    let gltf = gltf::Gltf::from_slice(glb).unwrap();
    gltf.document.nodes().any(|node| node.name() == Some(name))
}

/// The world-space vertex positions of the single primitive of the node named
/// `name`, reconstructed as `node_translation + local_position`.
fn node_world_positions(glb: &[u8], name: &str) -> Vec<[f64; 3]> {
    let gltf = gltf::Gltf::from_slice(glb).unwrap();
    let node = gltf
        .document
        .nodes()
        .find(|node| node.name() == Some(name))
        .unwrap_or_else(|| panic!("node {name} not found"));
    let (t, _rotation, _scale) = node.transform().decomposed();
    let mesh = node.mesh().expect("node has a mesh");
    let primitive = mesh.primitives().next().expect("mesh has a primitive");
    let reader = primitive.reader(|_| gltf.blob.as_deref());
    reader
        .read_positions()
        .expect("primitive has positions")
        .map(|p| {
            [
                f64::from(t[0]) + f64::from(p[0]),
                f64::from(t[1]) + f64::from(p[1]),
                f64::from(t[2]) + f64::from(p[2]),
            ]
        })
        .collect()
}

/// The radius of every cross-section ring in an envelope, in world units.
///
/// The envelope's vertices are grouped into consecutive rings of `SECTORS`
/// points; each ring's radius is the maximum distance of its points from their
/// common centroid.
fn ring_radii(positions: &[[f64; 3]]) -> Vec<f64> {
    positions
        .chunks(SECTORS as usize)
        .map(|ring| {
            #[allow(clippy::cast_precision_loss)]
            let n = ring.len() as f64;
            let centroid = [
                ring.iter().map(|p| p[0]).sum::<f64>() / n,
                ring.iter().map(|p| p[1]).sum::<f64>() / n,
                ring.iter().map(|p| p[2]).sum::<f64>() / n,
            ];
            ring.iter()
                .map(|p| {
                    let d = [p[0] - centroid[0], p[1] - centroid[1], p[2] - centroid[2]];
                    d[2].mul_add(d[2], d[1].mul_add(d[1], d[0] * d[0])).sqrt()
                })
                .fold(0.0_f64, f64::max)
        })
        .collect()
}

#[test]
fn lines_respect_the_budget_and_use_lines_mode() {
    let mut scene = zero_origin_scene();
    let trace = fixtures::collimated_bundle(R0, None);
    add_trace(&mut scene, &trace, 10, Envelope::None);

    let glb = scene.to_glb().unwrap();
    let gltf = gltf::Gltf::from_slice(&glb).unwrap();
    let node = gltf
        .document
        .nodes()
        .find(|node| node.name() == Some("collimated/lines"))
        .expect("lines node present");
    let primitive = node.mesh().unwrap().primitives().next().unwrap();
    assert_eq!(primitive.mode(), Mode::Lines);

    let reader = primitive.reader(|_| gltf.blob.as_deref());
    let segments = reader.read_indices().unwrap().into_u32().count() / 2;
    // Two stations means one segment per ray, so at most `max_lines` segments.
    assert!(segments >= 1);
    assert!(
        segments <= 10,
        "{segments} segments exceed the budget of 10"
    );
}

#[test]
fn max_lines_zero_draws_no_lines_node() {
    let mut scene = zero_origin_scene();
    let trace = fixtures::collimated_bundle(R0, None);
    add_trace(&mut scene, &trace, 0, Envelope::None);
    assert!(!has_node(&scene.to_glb().unwrap(), "collimated/lines"));
}

#[test]
fn mesh_envelope_is_exported_unchanged() {
    let mut scene = zero_origin_scene();
    let material = scene.add_material(Material::Translucent {
        color: [0.1, 0.2, 0.3, 0.4],
    });
    let world_positions = vec![
        Point3::new(0.001, 0.0, 0.0),
        Point3::new(0.002, 0.001, 0.05),
        Point3::new(-0.001, -0.002, 0.1),
    ];
    let envelope = TriMesh {
        patches: vec![SurfacePatch {
            name: Some("hand-built".to_string()),
            positions: world_positions.clone(),
            normals: None,
            colors: None,
            indices: vec![[0, 1, 2]],
            material,
        }],
    };
    let trace = fixtures::collimated_bundle(R0, None);
    add_trace(&mut scene, &trace, 0, Envelope::Mesh(envelope));

    let glb = scene.to_glb().unwrap();
    // Positions are unchanged except for the origin shift and the f32 cast.
    let got = node_world_positions(&glb, "collimated/envelope");
    assert_eq!(got.len(), world_positions.len());
    for (g, w) in got.iter().zip(&world_positions) {
        assert!((g[0] - w.x).abs() < 1e-6);
        assert!((g[1] - w.y).abs() < 1e-6);
        assert!((g[2] - w.z).abs() < 1e-6);
    }

    // The material is the mesh's own, not one injected by the crate.
    let gltf = gltf::Gltf::from_slice(&glb).unwrap();
    let node = gltf
        .document
        .nodes()
        .find(|node| node.name() == Some("collimated/envelope"))
        .unwrap();
    let primitive = node.mesh().unwrap().primitives().next().unwrap();
    let base = primitive
        .material()
        .pbr_metallic_roughness()
        .base_color_factor();
    assert!((f64::from(base[0]) - 0.1).abs() < 1e-6);
    assert!((f64::from(base[3]) - 0.4).abs() < 1e-6);
}

#[test]
fn none_envelope_draws_no_envelope_node() {
    let mut scene = zero_origin_scene();
    let trace = fixtures::collimated_bundle(R0, None);
    add_trace(&mut scene, &trace, 40, Envelope::None);
    assert!(!has_node(&scene.to_glb().unwrap(), "collimated/envelope"));
}

#[test]
fn default_round_collimated_has_constant_radius() {
    let mut scene = zero_origin_scene();
    let trace = fixtures::collimated_bundle(R0, None);
    add_trace(
        &mut scene,
        &trace,
        0,
        Envelope::DefaultRound {
            sectors: SECTORS,
            steps: STEPS,
        },
    );

    let positions = node_world_positions(&scene.to_glb().unwrap(), "collimated/envelope");
    assert!(!positions.is_empty());
    for p in &positions {
        let radius = p[0].hypot(p[1]);
        assert!(
            (radius - R0).abs() <= 0.01 * R0,
            "radius {radius} is not within 1% of {R0}"
        );
    }
}

#[test]
fn default_round_focus_pinches_to_a_small_radius() {
    let mut scene = zero_origin_scene();
    let trace = fixtures::focusing_bundle(R0);
    add_trace(
        &mut scene,
        &trace,
        0,
        Envelope::DefaultRound {
            sectors: SECTORS,
            steps: STEPS,
        },
    );

    let positions = node_world_positions(&scene.to_glb().unwrap(), "focus/envelope");
    let smallest = ring_radii(&positions)
        .into_iter()
        .fold(f64::INFINITY, f64::min);
    assert!(
        smallest < 0.2 * R0,
        "smallest ring radius {smallest} is not below {}",
        0.2 * R0
    );
}

#[test]
fn default_round_folded_makes_two_tubes() {
    let mut scene = zero_origin_scene();
    let trace = fixtures::folded_bundle(R0);
    add_trace(
        &mut scene,
        &trace,
        0,
        Envelope::DefaultRound {
            sectors: SECTORS,
            steps: STEPS,
        },
    );

    let glb = scene.to_glb().unwrap();
    let positions = node_world_positions(&glb, "folded/envelope");
    // Two segments, each a tube of `STEPS + 1` rings of `SECTORS` points.
    #[allow(clippy::cast_possible_truncation)]
    let positions_len = positions.len() as u32;
    assert_eq!(positions_len, 2 * (STEPS + 1) * SECTORS);

    let gltf = gltf::Gltf::from_slice(&glb).unwrap();
    let node = gltf
        .document
        .nodes()
        .find(|node| node.name() == Some("folded/envelope"))
        .unwrap();
    let primitive = node.mesh().unwrap().primitives().next().unwrap();
    let reader = primitive.reader(|_| gltf.blob.as_deref());
    #[allow(clippy::cast_possible_truncation)]
    let triangles = reader.read_indices().unwrap().into_u32().count() as u32 / 3;
    // Per segment: `STEPS` ring gaps, each `SECTORS` quads of two triangles.
    assert_eq!(triangles, 2 * STEPS * SECTORS * 2);
}

#[test]
fn default_round_vignetting_shrinks_the_later_segment() {
    let mut scene = zero_origin_scene();
    let trace = fixtures::vignetted_bundle(R0);
    add_trace(
        &mut scene,
        &trace,
        0,
        Envelope::DefaultRound {
            sectors: SECTORS,
            steps: STEPS,
        },
    );

    let positions = node_world_positions(&scene.to_glb().unwrap(), "vignetted/envelope");
    let per_segment = ((STEPS + 1) * SECTORS) as usize;
    assert_eq!(positions.len(), 2 * per_segment);

    let max_radius = |slice: &[[f64; 3]]| ring_radii(slice).into_iter().fold(0.0_f64, f64::max);
    let segment0 = max_radius(&positions[..per_segment]);
    let segment1 = max_radius(&positions[per_segment..]);
    assert!(
        segment1 < segment0,
        "segment 1 max radius {segment1} is not below segment 0 max radius {segment0}"
    );
}

#[test]
fn segment_with_fewer_than_three_valid_rays_is_skipped() {
    let mut scene = zero_origin_scene();
    // Width five, but only two rays valid at both stations: 2 < 3.
    let station0 = vec![
        Some(Point3::new(0.0, 0.0, 0.0)),
        Some(Point3::new(0.001, 0.0, 0.0)),
        None,
        None,
        None,
    ];
    let station1 = vec![
        Some(Point3::new(0.0, 0.0, 0.1)),
        Some(Point3::new(0.001, 0.0, 0.1)),
        None,
        None,
        None,
    ];
    let trace = RayTrace {
        uid: "sparse".to_string(),
        stations: vec![station0, station1],
        wavelength: None,
    };

    // The skipped segment is not an error.
    add_trace(&mut scene, &trace, 40, Envelope::default_round());

    let glb = scene.to_glb().unwrap();
    assert!(
        !has_node(&glb, "sparse/envelope"),
        "envelope should be absent"
    );
    // The two valid rays still produce lines.
    assert!(
        has_node(&glb, "sparse/lines"),
        "lines should still be drawn"
    );
}
