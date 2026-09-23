//! Diff tests: the update messages produced when one scene changes into another.
//!
//! Requires the `protocol` feature (see the crate's `[[test]]` entry).

use nalgebra::{Isometry3, Point3};
use optoscene::{
    diff, Envelope, Header, Layer, Material, MaterialId, MeshId, RayStyle, RayTrace, Scene,
    SceneNode, SceneOptions, SurfacePatch, TriMesh,
};

/// A scene whose origin is pinned, so geometry edits and node moves cannot shift
/// the effective origin (which would otherwise force a full resend).
fn pinned_scene() -> Scene {
    Scene::new(SceneOptions {
        origin: Some(Point3::origin()),
        ..SceneOptions::default()
    })
}

/// Registers a neutral opaque material.
fn opaque(scene: &mut Scene) -> MaterialId {
    scene.add_material(Material::Opaque {
        color: [0.8, 0.5, 0.5, 1.0],
        metallic: 0.0,
        roughness: 1.0,
    })
}

/// A single-triangle mesh shifted by `offset` along x (to vary its content).
fn triangle(material: MaterialId, offset: f64) -> TriMesh {
    TriMesh {
        patches: vec![SurfacePatch {
            name: None,
            positions: vec![
                Point3::new(offset, 0.0, 0.0),
                Point3::new(offset + 1.0, 0.0, 0.0),
                Point3::new(offset, 1.0, 0.0),
            ],
            normals: None,
            colors: None,
            indices: vec![[0, 1, 2]],
            material,
        }],
    }
}

/// Adds an optics node at world position `(x, 0, 0)`.
fn add_optic(scene: &mut Scene, uid: &str, mesh: MeshId, x: f64) {
    scene
        .add_node(SceneNode {
            uid: uid.to_string(),
            name: uid.to_string(),
            mesh: Some(mesh),
            transform: Isometry3::translation(x, 0.0, 0.0),
            layer: Layer::Optics,
            data: None,
        })
        .expect("node added");
}

/// A two-station ray bundle ending at `z1` (varying it changes the geometry).
fn trace(uid: &str, z1: f64) -> RayTrace {
    RayTrace {
        uid: uid.to_string(),
        stations: vec![
            vec![
                Some(Point3::new(0.0, 0.0, 0.0)),
                Some(Point3::new(0.001, 0.0, 0.0)),
                Some(Point3::new(0.0, 0.001, 0.0)),
            ],
            vec![
                Some(Point3::new(0.0, 0.0, z1)),
                Some(Point3::new(0.001, 0.0, z1)),
                Some(Point3::new(0.0, 0.001, z1)),
            ],
        ],
        wavelength: None,
    }
}

/// Adds the ray lines of `t` to the scene's `rays` layer.
fn add_rays(scene: &mut Scene, t: &RayTrace) {
    scene
        .add_ray_trace(
            t,
            &RayStyle {
                max_lines: 4,
                envelope: Envelope::None,
                ..RayStyle::default()
            },
        )
        .expect("ray trace added");
}

/// A short label for a header's kind, for order assertions.
const fn kind(header: &Header) -> &'static str {
    match header {
        Header::FullScene => "full",
        Header::RemoveNodes { .. } => "remove",
        Header::UpsertNodes { .. } => "upsert",
        Header::UpdateTransforms { .. } => "transform",
        Header::ReplaceLayer { .. } => "replace",
    }
}

#[test]
fn identical_scenes_produce_no_messages() {
    let build = || {
        let mut scene = pinned_scene();
        let material = opaque(&mut scene);
        let mesh = scene.add_mesh(triangle(material, 0.0)).unwrap();
        add_optic(&mut scene, "a", mesh, 0.0);
        scene
    };
    assert!(diff(&build(), &build()).unwrap().is_empty());
}

#[test]
fn moved_node_produces_only_update_transforms() {
    let mut old = pinned_scene();
    let material = opaque(&mut old);
    let mesh = old.add_mesh(triangle(material, 0.0)).unwrap();
    add_optic(&mut old, "a", mesh, 0.0);

    let mut new = pinned_scene();
    let material = opaque(&mut new);
    let mesh = new.add_mesh(triangle(material, 0.0)).unwrap();
    add_optic(&mut new, "a", mesh, 0.5);

    let messages = diff(&old, &new).unwrap();
    assert_eq!(messages.len(), 1);
    match &messages[0].header {
        Header::UpdateTransforms { nodes } => {
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].uid, "a");
            assert!((nodes[0].translation[0] - 0.5).abs() < 1e-6);
        }
        other => panic!("expected UpdateTransforms, got {other:?}"),
    }
    assert!(messages[0].payload.is_empty());
}

#[test]
fn changed_mesh_produces_upsert() {
    let mut old = pinned_scene();
    let material = opaque(&mut old);
    let mesh = old.add_mesh(triangle(material, 0.0)).unwrap();
    add_optic(&mut old, "a", mesh, 0.0);

    let mut new = pinned_scene();
    let material = opaque(&mut new);
    let mesh = new.add_mesh(triangle(material, 5.0)).unwrap();
    add_optic(&mut new, "a", mesh, 0.0);

    let messages = diff(&old, &new).unwrap();
    assert_eq!(messages.len(), 1);
    match &messages[0].header {
        Header::UpsertNodes { uids } => assert_eq!(uids, &["a".to_string()]),
        other => panic!("expected UpsertNodes, got {other:?}"),
    }
    assert!(!messages[0].payload.is_empty());
}

#[test]
fn removed_node_produces_remove() {
    let mut old = pinned_scene();
    let material = opaque(&mut old);
    let mesh = old.add_mesh(triangle(material, 0.0)).unwrap();
    add_optic(&mut old, "a", mesh, 0.0);
    add_optic(&mut old, "b", mesh, 2.0);

    let mut new = pinned_scene();
    let material = opaque(&mut new);
    let mesh = new.add_mesh(triangle(material, 0.0)).unwrap();
    add_optic(&mut new, "a", mesh, 0.0);

    let messages = diff(&old, &new).unwrap();
    assert_eq!(messages.len(), 1);
    match &messages[0].header {
        Header::RemoveNodes { uids } => assert_eq!(uids, &["b".to_string()]),
        other => panic!("expected RemoveNodes, got {other:?}"),
    }
}

#[test]
fn changed_ray_trace_produces_replace_layer() {
    let mut old = pinned_scene();
    let material = opaque(&mut old);
    let mesh = old.add_mesh(triangle(material, 0.0)).unwrap();
    add_optic(&mut old, "a", mesh, 0.0);
    add_rays(&mut old, &trace("beam", 0.1));

    let mut new = pinned_scene();
    let material = opaque(&mut new);
    let mesh = new.add_mesh(triangle(material, 0.0)).unwrap();
    add_optic(&mut new, "a", mesh, 0.0);
    add_rays(&mut new, &trace("beam", 0.2));

    let messages = diff(&old, &new).unwrap();
    assert_eq!(messages.len(), 1);
    match &messages[0].header {
        Header::ReplaceLayer { layer } => assert_eq!(layer, "rays"),
        other => panic!("expected ReplaceLayer, got {other:?}"),
    }
    assert!(!messages[0].payload.is_empty());
}

#[test]
fn changed_origin_produces_full_scene() {
    let build = |origin: Point3<f64>| {
        let mut scene = Scene::new(SceneOptions {
            origin: Some(origin),
            ..SceneOptions::default()
        });
        let material = opaque(&mut scene);
        let mesh = scene.add_mesh(triangle(material, 0.0)).unwrap();
        add_optic(&mut scene, "a", mesh, 0.0);
        scene
    };
    let old = build(Point3::origin());
    let new = build(Point3::new(1.0, 0.0, 0.0));

    let messages = diff(&old, &new).unwrap();
    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0].header, Header::FullScene));
    assert!(!messages[0].payload.is_empty());
}

#[test]
fn message_order_follows_the_spec() {
    let mut old = pinned_scene();
    let material = opaque(&mut old);
    let mesh_x = old.add_mesh(triangle(material, 0.0)).unwrap();
    add_optic(&mut old, "keep", mesh_x, 0.0);
    add_optic(&mut old, "move", mesh_x, 1.0);
    add_optic(&mut old, "drop", mesh_x, 2.0);
    add_rays(&mut old, &trace("beam", 0.1));

    let mut new = pinned_scene();
    let material = opaque(&mut new);
    let mesh_x = new.add_mesh(triangle(material, 0.0)).unwrap();
    let mesh_y = new.add_mesh(triangle(material, 5.0)).unwrap();
    add_optic(&mut new, "keep", mesh_y, 0.0); // content changed -> upsert
    add_optic(&mut new, "move", mesh_x, 1.5); // moved -> transform
                                              // "drop" is gone -> remove
    add_rays(&mut new, &trace("beam", 0.2)); // rays changed -> replace

    let messages = diff(&old, &new).unwrap();
    let kinds: Vec<&str> = messages.iter().map(|m| kind(&m.header)).collect();
    assert_eq!(kinds, ["remove", "upsert", "transform", "replace"]);
}
