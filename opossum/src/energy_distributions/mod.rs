use nalgebra::Point2;
use uom::si::f64::Energy;

pub mod general_gaussian;
pub mod uniform;

pub trait EnergyDistribution {
    fn apply(&self, input: &[Point2<f64>]) -> Vec<Energy>;

    fn get_total_energy(&self) -> Energy;
}

pub use general_gaussian::General2DGaussian;
pub use uniform::UniformDist;
// pub use hexapolar::Hexapolar;
// pub use random::Random;
// pub use sobol::SobolDist;
