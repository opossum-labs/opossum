//! glTF 2.0 export: a minimal schema subset, the [`builder`] that turns a
//! [`crate::Scene`] into it, and the [`container`] formats (GLB, `.gltf`+`.bin`).

pub mod builder;
pub mod container;
pub mod schema;
