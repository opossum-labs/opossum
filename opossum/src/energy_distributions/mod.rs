use nalgebra::Point2;
use uom::si::f64::Energy;

pub mod general_gaussian;
// pub mod uniform;

pub trait EnergyDistribution {
    fn apply(&self, total_energy: Energy, input: Vec<Point2<f64>>) -> Vec<Energy>;
}

// pub use fibonacci::{FibonacciEllipse, FibonacciRectangle};
// pub use grid::Grid;
// pub use hexapolar::Hexapolar;
// pub use random::Random;
// pub use sobol::SobolDist;
