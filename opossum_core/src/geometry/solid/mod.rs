#![warn(missing_docs)]
//! The solids a component can be made of.
//!
//! A [`Solid`] is either two faces capping a profile extruded along an axis ([`Extruded`]), the
//! intersection of the inner sides of several faces ([`ParametricSolid`]) or several solids sharing
//! interfaces ([`Assembly`]). Each kind lives in a file of its own, together with what it can do.
//!
//! A solid is described rather than built. Built at the placement of its node it becomes a
//! [`Placed`](crate::geometry::Placed) solid, which is what can answer geometric questions such as
//! whether a point lies inside it (see [`Body`]). Every kind answers them in its own file; the
//! [`Solid`] only hands a question on to the kind it holds.

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
    error::OpmResult,
    geometry::{
        Face, Placeable, Placement, Rim,
        body::{Body, BoundingBox},
    },
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

impl Solid {
    /// Return the face with the given index, as an [`Interface`] refers to it.
    ///
    /// # Arguments
    ///
    /// * `index` - for an [`Extruded`] solid 0 is the front and 1 the rear face; for a
    ///   [`ParametricSolid`] it is the position in its list of faces.
    ///
    /// # Returns
    ///
    /// The face, the frame it is stated in relative to the solid, and the side of it the solid lies
    /// on; or `None` if there is no such face. An [`Assembly`] has no faces of its own.
    fn face(&self, index: usize) -> Option<(&Face, Isometry, Side)> {
        match self {
            Self::Extruded(extruded) => extruded.face(index),
            Self::Parametric(parametric) => parametric.face(index),
            Self::Assembly(_) => None,
        }
    }
}

/// Which side of a face lies inside a [`ParametricSolid`], as
/// [`GeoSurface::is_behind`](crate::geometry::geo_surface::GeoSurface::is_behind) tells them apart.
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
    ///   [`GeoSurface::is_behind`](crate::geometry::geo_surface::GeoSurface::is_behind)). A point on
    ///   the face counts as behind it, so it is inside a solid lying behind the face and outside a
    ///   solid lying in front of it: two solids sharing a face do not both claim its points.
    const fn contains(self, is_behind: bool) -> bool {
        match self {
            Self::Behind => is_behind,
            Self::InFront => !is_behind,
        }
    }
}

impl Solid {
    /// Return the kind of solid this is, as the one thing the faces and the rim are asked of.
    const fn as_placeable(&self) -> &dyn Placeable {
        match self {
            Self::Extruded(extruded) => &**extruded,
            Self::Parametric(parametric) => parametric,
            Self::Assembly(assembly) => assembly,
        }
    }
    /// Return the kind of solid this is, as the one thing its volume is asked of.
    const fn as_body(&self) -> &dyn Body {
        match self {
            Self::Extruded(extruded) => &**extruded,
            Self::Parametric(parametric) => parametric,
            Self::Assembly(assembly) => assembly,
        }
    }
}

impl Placeable for Solid {
    fn faces(&self) -> Vec<(&Face, Isometry)> {
        self.as_placeable().faces()
    }
    fn rim(&self) -> Option<&Rim> {
        self.as_placeable().rim()
    }
}

impl Body for Solid {
    fn contains(&self, at: Placement<'_>, point: &Point3<Length>) -> bool {
        self.as_body().contains(at, point)
    }
    fn path_length_inside(&self, at: Placement<'_>, ray: &Ray) -> Option<Length> {
        self.as_body().path_length_inside(at, ray)
    }
    fn bounding_box(&self, at: Placement<'_>) -> OpmResult<BoundingBox> {
        self.as_body().bounding_box(at)
    }
    fn isometry(&self, at: Placement<'_>) -> Isometry {
        self.as_body().isometry(at)
    }
}

/// Collect the faces a solid numbers, one after the other until the numbering ends.
///
/// # Arguments
///
/// * `face` - the face with the given number, as [`Solid::face`] returns it.
///
/// # Returns
///
/// Every face with the frame it is stated in, in the order of the numbering.
fn collect_faces<'a>(
    face: impl Fn(usize) -> Option<(&'a Face, Isometry, Side)>,
) -> Vec<(&'a Face, Isometry)> {
    (0..)
        .map_while(face)
        .map(|(face, owner, _)| (face, owner))
        .collect()
}
