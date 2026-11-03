//! Core GLB export: roundtrip, container layout and mesh validation.

use nalgebra::{Isometry3, Point3, Vector3};
use optoscene::{
    Error, Layer, Material, MaterialId, Scene, SceneNode, SceneOptions, SurfacePatch, TriMesh,
};

/// Registers a neutral opaque material.
fn opaque(scene: &mut Scene) -> MaterialId {
    scene.add_material(Material::Opaque {
        color: [0.5, 0.5, 0.5, 1.0],
        metallic: 0.0,
        roughness: 1.0,
    })
}

/// A single-triangle patch, shifted by `offset` along x.
fn tri_patch(material: MaterialId, offset: f64) -> SurfacePatch {
    SurfacePatch {
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
    }
}

/// Adds a node referencing `mesh` at the given translation.
fn add_node(scene: &mut Scene, uid: &str, mesh: optoscene::MeshId, translation: [f64; 3]) {
    scene
        .add_node(SceneNode {
            uid: uid.to_string(),
            name: uid.to_string(),
            mesh: Some(mesh),
            transform: Isometry3::translation(translation[0], translation[1], translation[2]),
            layer: Layer::Optics,
            data: None,
        })
        .expect("node added");
}

#[test]
fn roundtrip_counts_and_positions() {
    // Pin the origin so the exported node translation matches the world value.
    let mut scene = Scene::new(SceneOptions {
        origin: Some(Point3::origin()),
        ..SceneOptions::default()
    });
    let material = opaque(&mut scene);
    let mesh = scene
        .add_mesh(TriMesh {
            patches: vec![tri_patch(material, 0.0)],
        })
        .unwrap();
    add_node(&mut scene, "node", mesh, [2.0, 3.0, 4.0]);

    let glb = scene.to_glb().unwrap();
    let gltf = gltf::Gltf::from_slice(&glb).unwrap();

    assert_eq!(gltf.document.meshes().count(), 1);
    // Exactly one content node carries the mesh (plus root and layer groups).
    assert_eq!(
        gltf.document
            .nodes()
            .filter(|node| node.mesh().is_some())
            .count(),
        1
    );

    let primitive = gltf
        .document
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .next()
        .unwrap();
    let reader = primitive.reader(|_| gltf.blob.as_deref());
    let positions: Vec<[f32; 3]> = reader.read_positions().unwrap().collect();
    assert_eq!(positions.len(), 3);
    assert!((positions[1][0] - 1.0).abs() < 1e-6);
    assert!((positions[2][1] - 1.0).abs() < 1e-6);

    let node = gltf
        .document
        .nodes()
        .find(|node| node.mesh().is_some())
        .unwrap();
    let (translation, _rotation, _scale) = node.transform().decomposed();
    assert!((translation[0] - 2.0).abs() < 1e-6);
    assert!((translation[1] - 3.0).abs() < 1e-6);
    assert!((translation[2] - 4.0).abs() < 1e-6);
}

#[test]
fn glb_header_length_and_chunk_alignment() {
    let mut scene = Scene::new(SceneOptions::default());
    let material = opaque(&mut scene);
    let mesh = scene
        .add_mesh(TriMesh {
            patches: vec![tri_patch(material, 0.0)],
        })
        .unwrap();
    add_node(&mut scene, "node", mesh, [0.0, 0.0, 0.0]);

    let glb = scene.to_glb().unwrap();
    assert_eq!(&glb[0..4], b"glTF");
    let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
    assert_eq!(total, glb.len());

    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    assert_eq!(json_len % 4, 0);
    assert_eq!(&glb[16..20], b"JSON");

    let bin_offset = 20 + json_len;
    let bin_len = u32::from_le_bytes(glb[bin_offset..bin_offset + 4].try_into().unwrap()) as usize;
    assert_eq!(bin_len % 4, 0);
    assert_eq!(&glb[bin_offset + 4..bin_offset + 8], b"BIN\0");
    assert_eq!(20 + json_len + 8 + bin_len, glb.len());
}

#[test]
fn export_is_byte_identical() {
    let build = || {
        let mut scene = Scene::new(SceneOptions::default());
        let material = opaque(&mut scene);
        let mesh = scene
            .add_mesh(TriMesh {
                patches: vec![tri_patch(material, 0.0), tri_patch(material, 2.0)],
            })
            .unwrap();
        add_node(&mut scene, "a", mesh, [0.0, 0.0, 0.0]);
        add_node(&mut scene, "b", mesh, [1.0, 0.0, 0.0]);
        scene.to_glb().unwrap()
    };
    assert_eq!(build(), build());
}

#[test]
fn out_of_range_index_is_invalid_mesh() {
    let mut scene = Scene::new(SceneOptions::default());
    let material = opaque(&mut scene);
    let mut patch = tri_patch(material, 0.0);
    patch.indices = vec![[0, 1, 5]];
    let err = scene
        .add_mesh(TriMesh {
            patches: vec![patch],
        })
        .unwrap_err();
    assert!(matches!(err, Error::InvalidMesh(_)));
}

#[test]
fn duplicate_uid_is_error() {
    let mut scene = Scene::new(SceneOptions::default());
    let material = opaque(&mut scene);
    let mesh = scene
        .add_mesh(TriMesh {
            patches: vec![tri_patch(material, 0.0)],
        })
        .unwrap();
    add_node(&mut scene, "dup", mesh, [0.0, 0.0, 0.0]);
    let err = scene
        .add_node(SceneNode {
            uid: "dup".to_string(),
            name: "dup".to_string(),
            mesh: Some(mesh),
            transform: Isometry3::identity(),
            layer: Layer::Optics,
            data: None,
        })
        .unwrap_err();
    assert!(matches!(err, Error::DuplicateUid(_)));
}

#[test]
fn zero_length_normal_is_invalid_mesh() {
    let mut scene = Scene::new(SceneOptions::default());
    let material = opaque(&mut scene);
    let patch = SurfacePatch {
        name: None,
        positions: vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ],
        // Finite (passes add_mesh) but degenerate: rejected at export.
        normals: Some(vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(0.0, 0.0, 1.0),
        ]),
        colors: None,
        indices: vec![[0, 1, 2]],
        material,
    };
    let mesh = scene
        .add_mesh(TriMesh {
            patches: vec![patch],
        })
        .unwrap();
    // The mesh must be referenced by a node to be exported (and thus normalized).
    add_node(&mut scene, "node", mesh, [0.0, 0.0, 0.0]);
    let err = scene.to_glb().unwrap_err();
    assert!(matches!(err, Error::InvalidMesh(_)));
}

#[test]
fn missing_normals_produce_no_normal_attribute() {
    let mut scene = Scene::new(SceneOptions::default());
    let material = opaque(&mut scene);
    let mesh = scene
        .add_mesh(TriMesh {
            patches: vec![tri_patch(material, 0.0)],
        })
        .unwrap();
    add_node(&mut scene, "node", mesh, [0.0, 0.0, 0.0]);

    let glb = scene.to_glb().unwrap();
    let gltf = gltf::Gltf::from_slice(&glb).unwrap();
    let primitive = gltf
        .document
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .next()
        .unwrap();
    let reader = primitive.reader(|_| gltf.blob.as_deref());
    assert!(reader.read_normals().is_none());
}

#[test]
fn provided_normals_are_written_verbatim() {
    let normals = vec![
        Vector3::new(0.0, 0.0, 1.0),
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
    ];
    let mut scene = Scene::new(SceneOptions::default());
    let material = opaque(&mut scene);
    let patch = SurfacePatch {
        name: None,
        positions: vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ],
        normals: Some(normals.clone()),
        colors: None,
        indices: vec![[0, 1, 2]],
        material,
    };
    let mesh = scene
        .add_mesh(TriMesh {
            patches: vec![patch],
        })
        .unwrap();
    add_node(&mut scene, "node", mesh, [0.0, 0.0, 0.0]);

    let glb = scene.to_glb().unwrap();
    let gltf = gltf::Gltf::from_slice(&glb).unwrap();
    let primitive = gltf
        .document
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .next()
        .unwrap();
    let reader = primitive.reader(|_| gltf.blob.as_deref());
    let written: Vec<[f32; 3]> = reader.read_normals().unwrap().collect();
    assert_eq!(written.len(), normals.len());
    #[allow(clippy::cast_possible_truncation)]
    for (got, expected) in written.iter().zip(&normals) {
        assert!((got[0] - expected.x as f32).abs() < 1e-6);
        assert!((got[1] - expected.y as f32).abs() < 1e-6);
        assert!((got[2] - expected.z as f32).abs() < 1e-6);
    }
}
