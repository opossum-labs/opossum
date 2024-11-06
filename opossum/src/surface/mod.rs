#![warn(missing_docs)]
//! Module for handling surfaces.
//!
//! OPOSSUM distiguishes between a geometric surface ([`GeoSurface`](crate::surface::geo_surface::GeoSurface)) which only handles the geometrical
//! math part and an [`OpticSurface`](crate::surface::optic_surface::OpticSurface).
//!
//! An [`OpticSurface`](crate::surface::optic_surface::OpticSurface) contains a [`GeoSurface`](crate::surface::geo_surface::GeoSurface) but also
//! adds further attributes such as a [`Coating`](crate::coatings::Coating) or an [`Aperture`](crate::aperture::Aperture).

mod cylinder;
mod parabola;
mod plane;
mod sphere;

pub mod geo_surface;
pub mod hit_map;
pub mod optic_surface;

pub use cylinder::Cylinder;
pub use parabola::Parabola;
pub use plane::Plane;
pub use sphere::Sphere;
