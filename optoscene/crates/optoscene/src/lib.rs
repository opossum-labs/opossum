//! `optoscene` — glTF/GLB export for the 3D visualization of optical systems.
//!
//! The crate turns ready-made geometry into glTF 2.0 (GLB or `.gltf` + `.bin`)
//! and makes that convenient for optical scenes. It is an **export** crate, not
//! a geometry crate: tessellation, normals and the true ray shape are supplied
//! by the caller, because only there are parametrization, apertures and
//! vignetting known.
//!
//! # Overview
//!
//! Build a [`Scene`], register [`Material`]s and [`TriMesh`]es, place
//! [`SceneNode`]s, then export:
//!
//! ```
//! use nalgebra::{Isometry3, Point3};
//! use optoscene::{Layer, Material, Scene, SceneNode, SceneOptions, SurfacePatch, TriMesh};
//!
//! let mut scene = Scene::new(SceneOptions::default());
//! let material = scene.add_material(Material::Opaque {
//!     color: [0.8, 0.2, 0.2, 1.0],
//!     metallic: 0.0,
//!     roughness: 0.6,
//! });
//! let mesh = scene
//!     .add_mesh(TriMesh {
//!         patches: vec![SurfacePatch {
//!             name: None,
//!             positions: vec![
//!                 Point3::new(0.0, 0.0, 0.0),
//!                 Point3::new(1.0, 0.0, 0.0),
//!                 Point3::new(0.0, 1.0, 0.0),
//!             ],
//!             normals: None,
//!             colors: None,
//!             indices: vec![[0, 1, 2]],
//!             material,
//!         }],
//!     })
//!     .unwrap();
//! scene
//!     .add_node(SceneNode {
//!         uid: "triangle-1".to_string(),
//!         name: "triangle".to_string(),
//!         mesh: Some(mesh),
//!         transform: Isometry3::identity(),
//!         layer: Layer::Optics,
//!         data: None,
//!     })
//!     .unwrap();
//! let glb = scene.to_glb().unwrap();
//! assert!(glb.starts_with(b"glTF"));
//! ```
//!
//! All lengths are in **meters** and all input coordinates are `f64`; the
//! conversion to `f32` happens only in the exporter. Output is deterministic:
//! identical input produces identical bytes.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod error;
mod gltf;
mod model;
mod rays;
mod scene;
mod util;

#[cfg(feature = "protocol")]
mod diff;

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;

pub use error::Error;
pub use model::{
    Envelope, Layer, Material, MaterialId, MeshId, RayStyle, RayTrace, SceneNode, SurfacePatch,
    TriMesh,
};
pub use scene::{Scene, SceneOptions};

#[cfg(feature = "protocol")]
pub use diff::{diff, full_scene, SceneMessage};
#[cfg(feature = "protocol")]
pub use optoscene_protocol::{Header, TransformEntry};
