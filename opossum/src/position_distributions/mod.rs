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
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

mod fibonacci;
mod grid;
mod hexagonal_tiling;
mod hexapolar;
// mod random;
mod sobol;

pub use fibonacci::{FibonacciEllipse, FibonacciRectangle};
pub use grid::Grid;
pub use hexagonal_tiling::HexagonalTiling;
pub use hexapolar::Hexapolar;
// pub use random::Random;
pub use sobol::SobolDist;

/// Trait for the generation of point distributions
pub trait PositionDistribution {
    /// Generate the point distribution.
    ///
    /// This function generates a vector of 3D points (of dimension [`Length`]) with the given parameters defined earlier.
    fn generate(&self) -> Vec<Point3<Length>>;
}

/// Enum for the different types of position distributions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PosDistType {
    // /// Rectangular, uniform random distribution
    // Random(random::Random),
    /// Rectangular, evenly-sized grid distribution
    Grid(grid::Grid),
    /// Hexagonal tiling distribution
    HexagonalTiling(hexagonal_tiling::HexagonalTiling),
    /// Hexapolar distribution
    Hexapolar(hexapolar::Hexapolar),
    /// Fibonacci rectangle distribution
    FibonacciRectangle(fibonacci::FibonacciRectangle),
    /// Fibonacci ellipse distribution
    FibonacciEllipse(fibonacci::FibonacciEllipse),
    /// Pseudo random Sobol distribution
    Sobol(sobol::SobolDist),
}
impl PosDistType {
    /// Generate the point distribution.
    #[must_use]
    pub fn generate(&self) -> &dyn PositionDistribution {
        match self {
            // Self::Random(dist) => dist,
            Self::Grid(dist) => dist,
            Self::HexagonalTiling(dist) => dist,
            Self::Hexapolar(dist) => dist,
            Self::FibonacciRectangle(dist) => dist,
            Self::FibonacciEllipse(dist) => dist,
            Self::Sobol(dist) => dist,
        }
    }
}
