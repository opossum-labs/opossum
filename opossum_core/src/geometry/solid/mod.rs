#![warn(missing_docs)]
//! The solids a component can be made of.
//!
//! A [`Solid`] is either two faces capping a profile extruded along an axis ([`Extruded`]), the
//! intersection of the inner sides of several faces ([`ParametricSolid`]) or several solids sharing
//! interfaces ([`Assembly`]). Each kind lives in a file of its own, together with what it can do:
//! where its faces end (see [`GeometryKind`]) and what it says about its volume (see [`Body`]).
//! The [`Solid`] only hands a question on to the kind it holds.

mod assembly;
mod extruded;
mod parametric;

pub use assembly::{Assembly, FaceId, Interface, Part};
pub use extruded::Extruded;
pub use parametric::ParametricSolid;

use nalgebra::Point3;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use crate::{
    error::OpossumError,
    geometry::{Face, Geometry, GeometryKind, body::Body},
    light::Ray,
    utils::geom_transformation::Isometry,
};

/// A component enclosing a volume of material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Solid {
    /// Two faces capping a profile extruded along an axis.
    Extruded(Box<Extruded>),
    /// The intersection of the inner sides of several faces.
    Parametric(ParametricSolid),
    /// Several solids sharing interfaces.
    Assembly(Assembly),
}

impl From<Extruded> for Solid {
    fn from(extruded: Extruded) -> Self {
        Self::Extruded(Box::new(extruded))
    }
}

impl TryFrom<Geometry> for Solid {
    type Error = OpossumError;

    /// Take the solid out of a geometry.
    ///
    /// # Errors
    ///
    /// This function returns an error if the geometry is a single surface, which encloses no
    /// volume.
    fn try_from(geometry: Geometry) -> Result<Self, Self::Error> {
        match geometry {
            Geometry::Solid(solid) => Ok(solid),
            Geometry::Surface(_) => Err(OpossumError::Other(
                "a single surface encloses no volume".into(),
            )),
        }
    }
}

impl Solid {
    /// Return the face with the given number and the side of it the solid lies on, as an
    /// [`Interface`] refers to it.
    ///
    /// # Arguments
    ///
    /// * `index` - for an [`Extruded`] solid 0 is the front and 1 the rear face; for a
    ///   [`ParametricSolid`] it is the position in its list of faces.
    ///
    /// # Returns
    ///
    /// The face and the side of it the solid lies on; or `None` if there is no such face. An
    /// [`Assembly`] has no faces of its own.
    fn face_and_side(&self, index: usize) -> Option<(&Face, Side)> {
        match self {
            Self::Extruded(extruded) => extruded.face_and_side(index),
            Self::Parametric(parametric) => parametric.face_and_side(index),
            Self::Assembly(_) => None,
        }
    }
    /// Return this solid moved by an isometry (see [`Geometry::moved`]).
    ///
    /// # Arguments
    ///
    /// * `by` - the frame this solid is stated in, given in the frame it is moved to.
    #[must_use]
    pub fn moved(&self, by: &Isometry) -> Self {
        match self {
            Self::Extruded(extruded) => Self::Extruded(Box::new(extruded.moved(by))),
            Self::Parametric(parametric) => Self::Parametric(parametric.moved(by)),
            Self::Assembly(assembly) => Self::Assembly(assembly.moved(by)),
        }
    }
    /// Return the kind of solid this is, as the one thing its faces are asked of.
    #[must_use]
    pub fn kind(&self) -> &dyn GeometryKind {
        match self {
            Self::Extruded(extruded) => &**extruded,
            Self::Parametric(parametric) => parametric,
            Self::Assembly(assembly) => assembly,
        }
    }
    /// Return the kind of solid this is, as the one thing its volume is asked of.
    #[must_use]
    pub fn as_body(&self) -> &dyn Body {
        match self {
            Self::Extruded(extruded) => &**extruded,
            Self::Parametric(parametric) => parametric,
            Self::Assembly(assembly) => assembly,
        }
    }
}

/// Which side of a face lies inside a [`ParametricSolid`], as
/// [`GeoSurface::is_behind`](crate::geometry::GeoSurface::is_behind) tells them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    /// The side the surface reports as behind it.
    Behind,
    /// The side the surface reports as in front of it.
    InFront,
}

impl Side {
    /// Return whether a point on the given side of a face lies inside the solid.
    ///
    /// # Arguments
    ///
    /// * `is_behind` - whether the face reports the point as behind it (see
    ///   [`GeoSurface::is_behind`](crate::geometry::GeoSurface::is_behind)). A point on
    ///   the face counts as behind it, so it is inside a solid lying behind the face and outside a
    ///   solid lying in front of it: two solids sharing a face do not both claim its points.
    const fn contains(self, is_behind: bool) -> bool {
        match self {
            Self::Behind => is_behind,
            Self::InFront => !is_behind,
        }
    }
}

/// Return where a ray meets the faces of a solid, each only where the solid says the face reaches.
///
/// # Arguments
///
/// * `faces` - the faces of the solid, in the order they are numbered.
/// * `ray` - the ray.
/// * `includes` - whether the face with the given number reaches the given point of its surface.
///
/// # Returns
///
/// The points where the ray meets a face of the solid.
fn face_hits<'a>(
    faces: impl Iterator<Item = &'a Face> + 'a,
    ray: &'a Ray,
    includes: impl Fn(usize, &Point3<Length>) -> bool + 'a,
) -> impl Iterator<Item = Point3<Length>> + 'a {
    faces.enumerate().filter_map(move |(index, face)| {
        face.intersect(ray)
            .map(|(point, _)| point)
            .filter(|point| includes(index, point))
    })
}
