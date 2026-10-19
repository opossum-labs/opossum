#![warn(missing_docs)]
//! Solids bounded by several faces, each stating which of its sides is inside.

use serde::{Deserialize, Serialize};

use super::Side;
use crate::{
    error::{OpmResult, OpossumError},
    geometry::Face,
    utils::geom_transformation::Isometry,
};

/// A solid bounded by several faces: the intersection of the inner sides of all of them.
///
/// Built from flat faces only, such a solid is always convex (prisms, corner cubes, roof prisms);
/// curved faces with their outside as inner side also allow concave shapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParametricSolid {
    faces: Vec<(Face, Side)>,
}

impl ParametricSolid {
    /// Create a new [`ParametricSolid`].
    ///
    /// # Arguments
    ///
    /// * `faces` - every bounding face, stated relative to the node, with the side that is inside.
    ///
    /// # Errors
    ///
    /// This function returns an error if no face is given.
    pub fn new(faces: Vec<(Face, Side)>) -> OpmResult<Self> {
        if faces.is_empty() {
            return Err(OpossumError::Other(
                "a parametric solid needs at least one bounding face".into(),
            ));
        }
        Ok(Self { faces })
    }
    /// See [`Solid::face`](super::Solid). The face at the given position in the list of faces.
    ///
    /// # Arguments
    ///
    /// * `index` - the position of the face.
    ///
    /// # Returns
    ///
    /// The face, the frame it is stated in (the solid's own), and the side of it that is inside; or
    /// `None` if there is no such face.
    pub(super) fn face(&self, index: usize) -> Option<(&Face, Isometry, Side)> {
        self.faces
            .get(index)
            .map(|(face, side)| (face, Isometry::identity(), *side))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_parametric_solid_needs_a_face() {
        assert!(ParametricSolid::new(vec![]).is_err());
    }
}
