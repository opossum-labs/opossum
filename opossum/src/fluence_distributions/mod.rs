//! Module for handling energy distributions
use nalgebra::Point2;
use uom::si::f64::Length;
pub mod general_gaussian;
use crate::nodes::fluence_detector::Fluence;

pub trait FluenceDistribution {
    fn apply(&self, input: &[Point2<Length>]) -> Vec<Fluence>;
    // fn renormalize(&self, energy_dist: &mut Vec<Energy>) {
    //     //sum up energy of rays that are valid: energy is larger than machine epsilon times total energy
    //     let min_energy = f64::EPSILON * self.get_total_energy();
    //     let total_energy_valid_rays = joule!(energy_dist
    //         .iter()
    //         .map(|e| {
    //             if *e > min_energy {
    //                 e.get::<joule>()
    //             } else {
    //                 0.
    //             }
    //         })
    //         .collect::<Vec<f64>>()
    //         .iter()
    //         .kahan_sum()
    //         .sum());
    //     //scaling factor if a significant amount of energy has been lost
    //     let energy_scale_factor = self.get_total_energy() / total_energy_valid_rays;
    //     let _ = energy_dist.iter_mut().map(|e| *e * energy_scale_factor);
    // }
}

// pub use general_gaussian::General2DGaussian;
// pub use uniform::UniformDist;

// use crate::{joule, nodes::fluence_detector::Fluence};
// pub use hexapolar::Hexapolar;
// pub use random::Random;
// pub use sobol::SobolDist;
