//! Export features layered on the core: materials + extensions, node metadata,
//! mesh instancing, layer selection, index width and the origin shift.

use nalgebra::{Isometry3, Point3};
use optoscene::{
    Layer, Material, MaterialId, Scene, SceneNode, SceneOptions, SurfacePatch, TriMesh,
};
use serde_json::Value;

/// Parses the JSON chunk of a GLB into a [`Value`].
fn glb_json(glb: &[u8]) -> Value {
    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&glb[20..20 + json_len]).unwrap()
}

/// Registers a neutral opaque material.
fn opaque(scene: &mut Scene) -> MaterialId {
    scene.add_material(Material::Opaque {
        color: [0.5, 0.5, 0.5, 1.0],
        metallic: 0.0,
        roughness: 1.0,
    })
}

/// A unit quad centered on the origin (so its AABB center is the node origin).
fn centered_quad(material: MaterialId) -> TriMesh {
    TriMesh {
        patches: vec![SurfacePatch {
            name: None,
            positions: vec![
                Point3::new(-0.5, -0.5, 0.0),
                Point3::new(0.5, -0.5, 0.0),
                Point3::new(0.5, 0.5, 0.0),
                Point3::new(-0.5, 0.5, 0.0),
            ],
            normals: None,
            colors: None,
            indices: vec![[0, 1, 2], [0, 2, 3]],
            material,
        }],
    }
}

/// Adds an optics-layer node at the identity transform.
fn add_optics_node(scene: &mut Scene, uid: &str, mesh: optoscene::MeshId) {
    scene
        .add_node(SceneNode {
            uid: uid.to_string(),
            name: uid.to_string(),
            mesh: Some(mesh),
            transform: Isometry3::identity(),
            layer: Layer::Optics,
            data: None,
        })
        .expect("node added");
}

#[test]
fn glass_material_has_all_extensions() {
    let mut scene = Scene::new(SceneOptions::default());
    let glass = scene.add_material(Material::Glass {
        color: [0.9, 0.95, 1.0],
        ior: 1.5,
        thickness: 0.01,
    });
    let mesh = scene.add_mesh(centered_quad(glass)).unwrap();
    add_optics_node(&mut scene, "glass", mesh);

    let json = glb_json(&scene.to_glb().unwrap());
    let extensions = &json["materials"][0]["extensions"];
    assert!(extensions.get("KHR_materials_transmission").is_some());
    assert!(extensions.get("KHR_materials_ior").is_some());
    assert!(extensions.get("KHR_materials_volume").is_some());
    assert_eq!(extensions["KHR_materials_ior"]["ior"], 1.5);

    let used: Vec<&str> = json["extensionsUsed"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(used.contains(&"KHR_materials_transmission"));
    assert!(used.contains(&"KHR_materials_ior"));
    assert!(used.contains(&"KHR_materials_volume"));

    // Fallback loading in other viewers must stay possible.
    assert!(json.get("extensionsRequired").is_none());
}

#[test]
fn iridescent_material_carries_the_iridescence_extension() {
    let mut scene = Scene::new(SceneOptions::default());
    let iridescent = scene.add_material(Material::Iridescent {
        color: [0.8, 0.8, 0.85],
        roughness: 0.1,
        iridescence: 1.0,
        iridescence_ior: 1.3,
        iridescence_thickness_min: 100.0,
        iridescence_thickness_max: 400.0,
    });
    let mesh = scene.add_mesh(centered_quad(iridescent)).unwrap();
    add_optics_node(&mut scene, "grating", mesh);

    let json = glb_json(&scene.to_glb().unwrap());
    let extension = &json["materials"][0]["extensions"]["KHR_materials_iridescence"];
    assert_eq!(extension["iridescenceFactor"], 1.0);
    assert_eq!(extension["iridescenceIor"], 1.3);
    assert_eq!(extension["iridescenceThicknessMinimum"], 100.0);
    assert_eq!(extension["iridescenceThicknessMaximum"], 400.0);

    let lists_iridescence = json["extensionsUsed"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .any(|ext| ext == "KHR_materials_iridescence");
    assert!(lists_iridescence, "iridescence missing from extensionsUsed");

    // The extension must not be required, so viewers that do not know it still load the mesh.
    assert!(json.get("extensionsRequired").is_none());
}

#[test]
fn identical_meshes_are_instanced() {
    let mut scene = Scene::new(SceneOptions::default());
    let material = opaque(&mut scene);
    let first = scene.add_mesh(centered_quad(material)).unwrap();
    let second = scene.add_mesh(centered_quad(material)).unwrap();
    assert_eq!(first, second);

    add_optics_node(&mut scene, "a", first);
    add_optics_node(&mut scene, "b", second);

    let gltf = gltf::Gltf::from_slice(&scene.to_glb().unwrap()).unwrap();
    assert_eq!(gltf.document.meshes().count(), 1);
    assert_eq!(
        gltf.document
            .nodes()
            .filter(|node| node.mesh().is_some())
            .count(),
        2
    );
}

#[test]
fn origin_defaults_to_aabb_center() {
    let mut scene = Scene::new(SceneOptions::default());
    let material = opaque(&mut scene);
    let mesh = scene.add_mesh(centered_quad(material)).unwrap();
    scene
        .add_node(SceneNode {
            uid: "far".to_string(),
            name: "far".to_string(),
            mesh: Some(mesh),
            transform: Isometry3::translation(1000.0, 0.0, 0.0),
            layer: Layer::Optics,
            data: None,
        })
        .unwrap();

    let json = glb_json(&scene.to_glb().unwrap());
    let nodes = json["nodes"].as_array().unwrap();

    let content = nodes
        .iter()
        .find(|node| node.get("mesh").is_some())
        .unwrap();
    for value in content["translation"].as_array().unwrap() {
        assert!(value.as_f64().unwrap().abs() < 1e-4);
    }

    let root = nodes
        .iter()
        .find(|node| node["name"] == "optoscene_root")
        .unwrap();
    let origin = root["extras"]["origin"].as_array().unwrap();
    assert!((origin[0].as_f64().unwrap() - 1000.0).abs() < 1e-6);
    assert!(origin[1].as_f64().unwrap().abs() < 1e-6);
    assert!(origin[2].as_f64().unwrap().abs() < 1e-6);
}

#[test]
fn node_extras_carry_uid_layer_and_data() {
    let mut scene = Scene::new(SceneOptions {
        origin: Some(Point3::origin()),
        ..SceneOptions::default()
    });
    let material = opaque(&mut scene);
    let mesh = scene.add_mesh(centered_quad(material)).unwrap();
    scene
        .add_node(SceneNode {
            uid: "node-uid".to_string(),
            name: "node".to_string(),
            mesh: Some(mesh),
            transform: Isometry3::identity(),
            layer: Layer::Optics,
            data: Some(serde_json::json!({ "focal_length_m": 0.1 })),
        })
        .unwrap();

    let glb = scene.to_glb().unwrap();
    let gltf = gltf::Gltf::from_slice(&glb).unwrap();
    let node = gltf
        .document
        .nodes()
        .find(|node| node.mesh().is_some())
        .unwrap();
    let raw = node.extras().as_ref().expect("extras present");
    let extras: Value = serde_json::from_str(raw.get()).unwrap();
    assert_eq!(extras["uid"], "node-uid");
    assert_eq!(extras["layer"], "optics");
    assert_eq!(extras["data"]["focal_length_m"], 0.1);
}

#[test]
fn index_type_switches_at_65536_vertices() {
    assert_eq!(index_component_type(&glb_with_vertices(65_535)), 5123);
    assert_eq!(index_component_type(&glb_with_vertices(65_536)), 5125);
}

/// The `componentType` of the SCALAR (index) accessor.
fn index_component_type(glb: &[u8]) -> u64 {
    let json = glb_json(glb);
    json["accessors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|accessor| accessor["type"] == "SCALAR")
        .unwrap()["componentType"]
        .as_u64()
        .unwrap()
}

/// Builds a GLB whose single patch has `count` vertices and one triangle.
fn glb_with_vertices(count: usize) -> Vec<u8> {
    let mut scene = Scene::new(SceneOptions::default());
    let material = opaque(&mut scene);
    let mut positions = vec![Point3::origin(); count];
    positions[1] = Point3::new(1.0, 0.0, 0.0);
    positions[2] = Point3::new(0.0, 1.0, 0.0);
    let mesh = scene
        .add_mesh(TriMesh {
            patches: vec![SurfacePatch {
                name: None,
                positions,
                normals: None,
                colors: None,
                indices: vec![[0, 1, 2]],
                material,
            }],
        })
        .unwrap();
    add_optics_node(&mut scene, "node", mesh);
    scene.to_glb().unwrap()
}

#[test]
fn layer_to_glb_excludes_other_layers() {
    let mut scene = Scene::new(SceneOptions {
        origin: Some(Point3::origin()),
        ..SceneOptions::default()
    });
    let material = opaque(&mut scene);
    let mesh = scene.add_mesh(centered_quad(material)).unwrap();
    for (uid, layer) in [("optic", Layer::Optics), ("aux", Layer::Aux)] {
        scene
            .add_node(SceneNode {
                uid: uid.to_string(),
                name: uid.to_string(),
                mesh: Some(mesh),
                transform: Isometry3::identity(),
                layer,
                data: None,
            })
            .unwrap();
    }

    let json = glb_json(&scene.layer_to_glb(Layer::Optics).unwrap());
    let nodes = json["nodes"].as_array().unwrap();
    let layers: Vec<&str> = nodes
        .iter()
        .filter_map(|node| node.get("extras")?.get("layer")?.as_str())
        .collect();
    assert!(layers.contains(&"optics"));
    assert!(!layers.contains(&"aux"));
}
