//! Turning an optical model into a 3D scene.
//!
//! The geometry itself comes from `opossum_core`, which describes the shape of every component, and
//! the file format from `optoscene`, which knows glTF. This crate is the seam between the two: it
//! decides what of a model gets drawn at all, turns each shape into triangles, and chooses the
//! material it is drawn in.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod appearance;
mod components;
mod hash;
mod manifest;
mod material;
mod mesh;
mod rays;
mod sources;
#[cfg(test)]
mod test_helpers;

pub use components::{glb_of_node, scene_glb};
pub use manifest::manifest_of;
pub use rays::{LineColors, axis_glb, light_glb};
pub use sources::{SpectrumDrawing, source_wavelength_range};

use nalgebra::Point3;
use opossum_core::error::{OpmResult, OpossumError};
use optoscene::{Scene, SceneOptions};

/// Return an empty scene pinned at the coordinate origin, which keeps what it holds where it is
/// given. Left to itself the exporter would shift it by the center of its bounding box.
fn pinned_scene() -> Scene {
    Scene::new(SceneOptions {
        origin: Some(Point3::origin()),
        ..SceneOptions::default()
    })
}

/// Write a scene as a binary glTF file.
///
/// # Errors
///
/// This function returns an error if `optoscene` cannot write the scene.
fn glb(scene: &Scene) -> OpmResult<Vec<u8>> {
    scene
        .to_glb()
        .map_err(|e| OpossumError::Other(format!("the scene could not be written: {e}")))
}
