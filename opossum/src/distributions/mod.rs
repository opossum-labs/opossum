use nalgebra::Point3;
use uom::si::f64::Length;

pub mod fibonacci;
pub mod grid;
pub mod hexapolar;
pub mod random;
pub mod sobol;

pub use fibonacci::{FibonacciEllipse, FibonacciRectangle};
pub use grid::Grid;
pub use hexapolar::Hexapolar;
pub use random::Random;
pub use sobol::SobolDist;

pub trait Distribution {
    fn generate(&self) -> Vec<Point3<Length>>;
}
