//! Module for handling energy distributions
pub mod general_gaussian;
pub mod uniform;
use std::fmt::Display;

pub use general_gaussian::General2DGaussian;
use serde::{Deserialize, Serialize};
use strum::EnumIter;
pub use uniform::UniformDist;

use crate::{error::OpmResult, joule};
use kahan::KahanSummator;
use nalgebra::Point2;
use uom::si::f64::{Energy, Length};

pub trait EnergyDistribution {
    fn apply(&self, input: &[Point2<Length>]) -> Vec<Energy>;
    fn get_total_energy(&self) -> Energy;
    fn renormalize(&self, energy_dist: &mut Vec<Energy>) {
        //sum up energy of rays that are valid: energy is larger than machine epsilon times total energy
        let min_energy = f64::EPSILON * self.get_total_energy();
        let total_energy_valid_rays = joule!(
            energy_dist
                .iter()
                .map(|e| {
                    if *e > min_energy {
                        e.get::<joule>()
                    } else {
                        0.
                    }
                })
                .collect::<Vec<f64>>()
                .iter()
                .kahan_sum()
                .sum()
        );
        //scaling factor if a significant amount of energy has been lost
        let energy_scale_factor = self.get_total_energy() / total_energy_valid_rays;
        let _ = energy_dist.iter_mut().map(|e| *e * energy_scale_factor);
    }
}

impl Default for EnergyDistType {
    fn default() -> Self {
        Self::Uniform(UniformDist::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter, Copy)]
pub enum EnergyDistType {
    Uniform(UniformDist),
    General2DGaussian(general_gaussian::General2DGaussian),
}

impl EnergyDistType {
    #[must_use]
    pub fn generate(&self) -> &dyn EnergyDistribution {
        match self {
            Self::Uniform(dist) => dist,
            Self::General2DGaussian(dist) => dist,
        }
    }

    pub fn set_energy(&mut self, energy: Energy) -> OpmResult<()> {
        match self {
            EnergyDistType::Uniform(uniform_dist) => uniform_dist.set_energy(energy)?,
            EnergyDistType::General2DGaussian(general2_dgaussian) => {
                general2_dgaussian.set_energy(energy)?
            }
        };
        Ok(())
    }

    pub fn default_from_name(name: &str) -> Option<Self> {
        match name {
            "Uniform" => Some(UniformDist::default().into()),
            "Generalized Gaussian" => Some(General2DGaussian::default().into()),
            _ => None,
        }
    }
}

impl Display for EnergyDistType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dist_string = match self {
            Self::Uniform(_) => "Uniform",
            Self::General2DGaussian(_) => "Generalized Gaussian",
        };
        write!(f, "{dist_string}")
    }
}
