#![warn(missing_docs)]
//! Module for handling surfaces.
//!
//! OPOSSUM distiguishes between a geometric surface ([`GeoSurface`]) which only handles the geometrical math part and an [`OpticalSurface`].
//!
//! An [`OpticalSurface`] contains a [`GeoSurface`] but also adds further attributes such as a [`Coating`](crate::coatings::Coating) and an
//! [`Aperture`](crate::aperture::Aperture).

mod cylinder;
mod parabola;
mod plane;
mod sphere;

pub mod geo_surface;
pub mod hit_map;
pub mod optical_surface;

pub use cylinder::Cylinder;
pub use parabola::Parabola;
pub use plane::Plane;
pub use sphere::Sphere;
