#![warn(missing_docs)]
//! Solids bounded by several faces, each stating which of its sides is inside.

use serde::{Deserialize, Serialize};

use super::{Side, collect_faces};
use crate::{
    error::{OpmResult, OpossumError},
    geometry::{Face, Placeable, Rim},
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

impl Placeable for ParametricSolid {
    fn faces(&self) -> Vec<(&Face, Isometry)> {
        collect_faces(|index| self.face(index))
    }
    /// A parametric solid has no lateral boundary: its faces bound it by themselves.
    fn rim(&self) -> Option<&Rim> {
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        geometry::{Placed, SurfaceShape},
        millimeter,
    };
    use approx::assert_abs_diff_eq;
    use uom::si::length::millimeter;

    #[test]
    fn a_parametric_solid_needs_a_face() {
        assert!(ParametricSolid::new(vec![]).is_err());
    }
    /// A placed parametric solid has one surface per face, in the order the faces were given, and no
    /// rim: its faces bound it by themselves.
    #[test]
    fn a_placed_parametric_solid_numbers_its_faces_as_given() -> OpmResult<()> {
        let at = |z: f64| Isometry::new_along_z(millimeter!(z));
        let solid = ParametricSolid::new(vec![
            (Face::new(SurfaceShape::Plane, at(0.0)?), Side::Behind),
            (Face::new(SurfaceShape::Plane, at(10.0)?), Side::InFront),
            (
                Face::new(
                    SurfaceShape::Sphere {
                        radius: millimeter!(40.0),
                    },
                    at(2.0)?,
                ),
                Side::Behind,
            ),
        ])?;
        let frame = Isometry::new_along_z(millimeter!(100.0))?;
        let placed = Placed::new(solid, &frame)?;
        assert_eq!(placed.face_count(), 3);
        assert!(placed.rim().is_none());
        let expected_z = [0.0, 10.0, 42.0];
        for (index, z) in expected_z.into_iter().enumerate() {
            assert_abs_diff_eq!(
                placed.anchor(index)?.translation().z.get::<millimeter>(),
                z,
                epsilon = 1e-9
            );
            let surface = placed
                .surface(index)
                .ok_or_else(|| OpossumError::Other(format!("face {index} has no surface")))?;
            assert_eq!(*surface.0.isometry(), frame.append(&placed.anchor(index)?));
        }
        Ok(())
    }
}
