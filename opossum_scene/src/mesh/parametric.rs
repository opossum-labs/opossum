//! Drawing a solid bounded by its faces alone.

use opossum_core::{
    apertures::Outline,
    error::{OpmResult, OpossumError},
    geometry::ParametricSolid,
};

use super::{Meshable, surface_mesh::FacePatch};

/// The faces of a parametric solid end where they meet each other, which no outline states, so it
/// cannot be meshed yet.
///
/// # Errors
///
/// This function always returns an error.
impl Meshable for ParametricSolid {
    fn mesh(&self, _segments: usize, _unbounded: &Outline) -> OpmResult<Vec<FacePatch>> {
        Err(OpossumError::Other(
            "a parametric solid cannot be meshed yet".into(),
        ))
    }
}
