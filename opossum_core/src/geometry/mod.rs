#![warn(missing_docs)]
//! Module for handling surfaces.
//!
//! OPOSSUM distinguishes between a [`Face`], an unbounded surface of a given [`SurfaceShape`] at a
//! frame which only handles the geometrical math part, and an
//! [`OpticSurface`](crate::core_optics::optic_surface::OpticSurface).
//!
//! An [`OpticSurface`](crate::core_optics::optic_surface::OpticSurface) contains a [`Face`] but
//! also adds further attributes such as a [`Coating`](crate::coatings::Coating) or the [`Rim`] that
//! bounds it.
//!
//! While a [`Face`] is an unbounded interface, a [`Body`](crate::geometry::body::Body) is a closed
//! volume bounded by such faces. It is the domain volumetric quantities are defined on.

mod face;
mod node_geometry;
mod rim;
mod solid;
mod surface;

pub mod body;

pub use face::Face;
pub use node_geometry::{Geometry, GeometryKind, SurfaceExtent, SurfaceGeometry, SurfaceHit};
pub use rim::Rim;
pub use solid::{Assembly, Extruded, FaceId, Interface, ParametricSolid, Part, Side};
pub use surface::{Cylinder, GeoSurface, Parabola, Plane, Sphere, SurfaceShape};
