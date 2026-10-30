//! Turning an optical model into a 3D scene.
//!
//! The geometry itself comes from `opossum_core`, which knows the shape of every component, and the
//! file format from `optoscene`, which knows glTF. This crate is only the seam between the two: it
//! decides what of a model gets drawn at all, and which material it is drawn in.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod appearance;
mod components;
mod hash;
mod manifest;
mod material;
mod mesh;
mod rays;
#[cfg(test)]
mod test_helpers;

pub use components::{glb_of_node, scene_glb};
pub use manifest::manifest_of;
pub use rays::{AXIS_COLOR, rays_glb};
