#![warn(missing_docs)]
//! Module for handling the shape of components.
//!
//! A [`Face`] is an unbounded surface of a given [`SurfaceShape`] at a frame, which only handles
//! the geometrical math part. A [`Geometry`] is a component made of numbered faces and says where
//! each of them ends, for instance at a [`Rim`]; asked where a ray hits one of its faces, it answers
//! with a [`SurfaceHit`] (see [`Geometry::intersect`]).
//!
//! An [`OpticSurface`](crate::core_optics::optic_surface::OpticSurface) is one face of the geometry
//! of a node, named by its number, with further attributes such as a
//! [`Coating`](crate::coatings::Coating). It holds no geometry itself.
//!
//! While a [`Face`] is an unbounded interface, a [`Body`](crate::geometry::body::Body) is a closed
//! volume bounded by such faces. It is the domain volumetric quantities are defined on.

mod face;
mod mesh;
mod node_geometry;
mod rim;
mod solid;
mod surface;

pub mod body;

pub use face::{Face, FaceIndex};
pub use mesh::{FacePatch, SurfaceMesh};
pub use node_geometry::{Geometry, GeometryKind, SurfaceExtent, SurfaceGeometry, SurfaceHit};
pub use rim::Rim;
pub use solid::{Assembly, Extruded, FaceId, Interface, ParametricSolid, Part, Side};
pub use surface::{Cylinder, GeoSurface, Parabola, Plane, Sphere, SurfaceShape};
