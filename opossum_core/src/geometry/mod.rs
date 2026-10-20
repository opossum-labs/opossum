#![warn(missing_docs)]
//! Module for handling surfaces.
//!
//! OPOSSUM distinguishes between a geometric surface ([`GeoSurface`](crate::geometry::geo_surface::GeoSurface)) which only handles the geometrical
//! math part and an [`OpticSurface`](crate::core_optics::optic_surface::OpticSurface).
//!
//! An [`OpticSurface`](crate::core_optics::optic_surface::OpticSurface) contains a [`GeoSurface`](crate::geometry::geo_surface::GeoSurface) but also
//! adds further attributes such as a [`Coating`](crate::coatings::Coating) or the [`Rim`] that bounds it.
//!
//! While a [`GeoSurface`](crate::geometry::geo_surface::GeoSurface) is an unbounded interface, a
//! [`Body`](crate::geometry::body::Body) is a closed volume bounded by such surfaces. It is the
//! domain volumetric quantities are defined on.

mod boundary;
mod cylinder;
mod face;
mod node_geometry;
mod parabola;
mod placed;
mod plane;
mod rim;
mod solid;
mod sphere;

pub mod body;
pub mod geo_surface;

pub use boundary::FaceBoundary;
pub use cylinder::Cylinder;
pub use face::{Face, SurfaceShape};
pub use node_geometry::{Geometry, SurfaceExtent, SurfaceGeometry};
pub use parabola::Parabola;
pub use placed::{Placeable, Placed, Placement};
pub use plane::Plane;
pub use rim::Rim;
pub use solid::{Assembly, Extruded, FaceId, Interface, ParametricSolid, Part, Side, Solid};
pub use sphere::Sphere;
