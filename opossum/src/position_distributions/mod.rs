#![warn(missing_docs)]
//! Module for handling position distributions
//!
//! These distribution are mainly used during the construction of [`Ray`](crate::ray::Ray) bundles ([`Rays`](crate::rays::Rays)).
//!
//! ## Example
//!
//! ```rust
//! use opossum::{millimeter, position_distributions::{PositionDistribution, Random}};
//!
//! let grid=Random::new(
//!   millimeter!(1.0),
//!   millimeter!(2.0),
//!   10).unwrap();
//! let points=grid.generate();
//! assert_eq!(points.len(), 10);
//! ```
//! `points` now contains a vector of 10 randomly-placed 3D points within the rectangle (-0.5 mm .. 0.5 mm) x (-1.0 mm .. 1.0 mm).
use nalgebra::Point3;
use uom::si::f64::Length;

mod fibonacci;
mod grid;
mod hexapolar;
mod random;
mod sobol;

pub use fibonacci::{FibonacciEllipse, FibonacciRectangle};
pub use grid::Grid;
pub use hexapolar::Hexapolar;
pub use random::Random;
pub use sobol::SobolDist;

/// Trait for the generation of point distributions
pub trait PositionDistribution {
    /// Generate the point distribution.
    ///
    /// This function generates a vector of 3D points (of dimension [`Length`]) with the given parameters defined earlier.
    fn generate(&self) -> Vec<Point3<Length>>;
}
