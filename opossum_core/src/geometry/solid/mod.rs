#![warn(missing_docs)]
//! The kinds of geometry that enclose a volume of material.
//!
//! A solid is either two faces capping a profile extruded along an axis ([`Extruded`]), the
//! intersection of the inner sides of several faces ([`ParametricSolid`]) or several solids sharing
//! interfaces ([`Assembly`]). Each kind lives in a file of its own, together with what it can do:
//! where its faces end (see [`GeometryKind`](crate::geometry::GeometryKind)) and what it says about
//! its volume (see [`Body`](crate::geometry::body::Body)).

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
    geometry::{Face, FaceIndex},
    light::Ray,
};

/// Which side of a face lies inside a solid, as
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
    includes: impl Fn(FaceIndex, &Point3<Length>) -> bool + 'a,
) -> impl Iterator<Item = Point3<Length>> + 'a {
    faces.enumerate().filter_map(move |(index, face)| {
        face.intersect(ray)
            .map(|(point, _)| point)
            .filter(|point| includes(FaceIndex::new(index), point))
    })
}
