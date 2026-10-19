#![warn(missing_docs)]
//! A description built at the placement of its node.
//!
//! A [`Geometry`](crate::geometry::Geometry) is a description: faces stated relative to the node.
//! [`Placed`] is that description together with the placement of the node and the geometric surface
//! of every face, which is what rays are traced against. The description stays the single truth;
//! the surfaces are what building it at a placement yields, and every other fact about a face — its
//! anchor, the rim bounding it — is derived from the description when it is asked for.
//!
//! The faces of a placed thing are numbered. What a face means (which one light enters through, which
//! one it leaves through) is not decided here: that is up to the node using it.

use crate::{
    error::{OpmResult, OpossumError},
    geometry::{Face, Rim, geo_surface::GeoSurfaceRef},
    utils::geom_transformation::Isometry,
};

/// A description that can be built at the placement of a node: it states its faces and the rim that
/// bounds them.
pub trait Placeable {
    /// Return every face of this description, in the order they are numbered.
    ///
    /// # Returns
    ///
    /// Each face with the frame it is stated in, relative to the node.
    fn faces(&self) -> Vec<(&Face, Isometry)>;
    /// Return the lateral boundary of the faces, or `None` if they are unbounded.
    fn rim(&self) -> Option<&Rim>;
}

/// A description built at the placement of its node.
///
/// It keeps the description next to what building adds: the placement of the node and one geometric
/// surface per face, in the order of [`Placeable::faces`]. The surfaces are the ones a node traces
/// its rays at, shared rather than built a second time.
#[derive(Debug, Clone)]
pub struct Placed<T> {
    description: T,
    node_frame: Isometry,
    surfaces: Vec<GeoSurfaceRef>,
}

impl<T> Placed<T> {
    /// Return the description this was built from.
    #[must_use]
    pub const fn description(&self) -> &T {
        &self.description
    }
    /// Return the placement of the node this was built at.
    #[must_use]
    pub const fn node_frame(&self) -> &Isometry {
        &self.node_frame
    }
    /// Return the geometric surface of every face, in the order the faces are numbered.
    #[must_use]
    pub fn surfaces(&self) -> &[GeoSurfaceRef] {
        &self.surfaces
    }
    /// Assemble a [`Placed`] from its parts.
    ///
    /// # Arguments
    ///
    /// * `description` - the description.
    /// * `node_frame` - the placement of the node the surfaces were built at.
    /// * `surfaces` - one surface per face of `description`, in the order of its faces.
    pub(crate) const fn from_parts(
        description: T,
        node_frame: Isometry,
        surfaces: Vec<GeoSurfaceRef>,
    ) -> Self {
        Self {
            description,
            node_frame,
            surfaces,
        }
    }
    /// Take this [`Placed`] apart into the description, the placement and the surfaces.
    pub(crate) fn into_parts(self) -> (T, Isometry, Vec<GeoSurfaceRef>) {
        (self.description, self.node_frame, self.surfaces)
    }
}

impl<T: Placeable> Placed<T> {
    /// Build a description at the placement of its node.
    ///
    /// # Arguments
    ///
    /// * `description` - what to build.
    /// * `node_frame` - the placement of the node.
    ///
    /// # Errors
    ///
    /// This function returns an error if a face cannot be built (see [`Face::place`]).
    pub fn new(description: T, node_frame: &Isometry) -> OpmResult<Self> {
        let surfaces = description
            .faces()
            .into_iter()
            .map(|(face, owner)| face.place(node_frame, &owner))
            .collect::<OpmResult<Vec<_>>>()?;
        Ok(Self {
            description,
            node_frame: *node_frame,
            surfaces,
        })
    }
    /// Return the number of faces.
    #[must_use]
    pub fn face_count(&self) -> usize {
        self.surfaces.len()
    }
    /// Return the geometric surface of the face with the given number.
    ///
    /// # Returns
    ///
    /// The surface, or `None` if there is no such face.
    #[must_use]
    pub fn surface(&self, index: usize) -> Option<&GeoSurfaceRef> {
        self.surfaces.get(index)
    }
    /// Return the anchor of the face with the given number: the frame of its geometric surface
    /// relative to the node.
    ///
    /// # Errors
    ///
    /// This function returns an error if there is no such face.
    pub fn anchor(&self, index: usize) -> OpmResult<Isometry> {
        let faces = self.description.faces();
        let (face, owner) = faces.get(index).ok_or_else(|| {
            OpossumError::Other(format!(
                "there is no face {index}: the geometry has {} faces",
                faces.len()
            ))
        })?;
        face.anchor(owner)
    }
    /// Return the lateral boundary of the faces, stated relative to the node, or `None` if they are
    /// unbounded.
    #[must_use]
    pub fn rim(&self) -> Option<&Rim> {
        self.description.rim()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType},
        degree,
        geometry::{Extruded, Geometry, SurfaceShape},
        millimeter,
        types::validated_type_definitions::ValidatedCrossSection,
    };

    fn lens() -> OpmResult<Geometry> {
        Geometry::singlet(
            millimeter!(50.0),
            millimeter!(-80.0),
            millimeter!(5.0),
            ValidatedCrossSection::try_new(Aperture::new_circle(
                millimeter!(10.0),
                ApertureType::Hole,
                None,
            )?)?,
        )
    }
    #[test]
    fn a_placed_description_has_one_surface_per_face() -> OpmResult<()> {
        let placed = lens()?.place(&Isometry::identity())?;
        assert_eq!(placed.face_count(), placed.description().faces().len());
        assert_eq!(placed.face_count(), 2);
        assert_eq!(placed.surfaces().len(), 2);
        Ok(())
    }
    /// A face is built at the node followed by its anchor, so the anchor taken from the description
    /// is where the surface of the same number sits relative to the node.
    #[test]
    fn every_surface_sits_at_the_node_followed_by_the_anchor_of_its_face() -> OpmResult<()> {
        let node_frame = Isometry::new(millimeter!(1.0, -2.0, 30.0), degree!(10.0, 5.0, 0.0))?;
        assert_placed_at_anchors(&lens()?.place(&node_frame)?)?;
        // The faces of an extrusion are stated in its axis, which is turned against the node.
        let turned = Extruded::new(
            Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?,
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(20.0))?,
            ),
            ValidatedCrossSection::try_new(Aperture::new_circle(
                millimeter!(10.0),
                ApertureType::Hole,
                None,
            )?)?,
        )?;
        assert_placed_at_anchors(&Placed::new(turned, &node_frame)?)
    }
    /// Assert that the surface of every face sits at the node followed by the anchor of that face.
    fn assert_placed_at_anchors<T: Placeable>(placed: &Placed<T>) -> OpmResult<()> {
        for index in 0..placed.face_count() {
            let surface = placed
                .surface(index)
                .ok_or_else(|| OpossumError::Other(format!("face {index} has no surface")))?;
            assert_eq!(
                *surface.0.isometry(),
                placed.node_frame().append(&placed.anchor(index)?)
            );
        }
        Ok(())
    }
    #[test]
    fn a_face_that_does_not_exist_has_neither_a_surface_nor_an_anchor() -> OpmResult<()> {
        let placed = lens()?.place(&Isometry::identity())?;
        assert!(placed.surface(2).is_none());
        let error = placed.anchor(2).unwrap_err();
        assert!(error.to_string().contains("no face 2"), "{error}");
        Ok(())
    }
}
