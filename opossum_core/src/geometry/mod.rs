#![warn(missing_docs)]
//! Module for handling surfaces.
//!
//! OPOSSUM distiguishes between a geometric surface ([`GeoSurface`](crate::geometry::geo_surface::GeoSurface)) which only handles the geometrical
//! math part and an [`OpticSurface`](crate::core_optics::optic_surface::OpticSurface).
//!
//! An [`OpticSurface`](crate::core_optics::optic_surface::OpticSurface) contains a [`GeoSurface`](crate::geometry::geo_surface::GeoSurface) but also
//! adds further attributes such as a [`Coating`](crate::coatings::Coating) or an [`Aperture`](crate::apertures::Aperture).
//!
//! While a [`GeoSurface`](crate::geometry::geo_surface::GeoSurface) is an unbounded interface, a
//! [`Body`](crate::geometry::body::Body) is a closed volume bounded by such surfaces. It is the
//! domain volumetric quantities are defined on.

mod cylinder;
mod face;
mod node_geometry;
mod parabola;
mod plane;
mod rim;
mod sphere;

pub mod body;
pub mod geo_surface;

pub use cylinder::Cylinder;
pub use face::{Face, SurfaceShape};
pub use node_geometry::{
    Assembly, Extruded, FaceId, Geometry, Interface, ParametricSolid, Part, PlacedSurface, Side,
    Solid, SurfaceGeometry,
};
pub use parabola::Parabola;
pub use plane::Plane;
pub use rim::{Rim, RimRole};
pub use sphere::Sphere;
