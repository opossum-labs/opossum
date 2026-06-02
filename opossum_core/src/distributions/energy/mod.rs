//! Energy distribution functions
//!
//! These functions are used while generating ray bundles ([`Rays`](crate::light::Rays)).
pub mod general_gaussian;
pub mod uniform;
use std::fmt::Display;

pub use general_gaussian::General2DGaussian;
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use strum::EnumIter;
pub use uniform::UniformDist;

use crate::{error::OpmResult, joule, utils::default_from_name::DefaultFromName};
use kahan::KahanSummator;
use nalgebra::Point2;
use uom::si::f64::{Energy, Length};

pub trait EnergyDistribution {
    /// Applies the energy distribution logic to a set of spatial points.
    ///
    /// This function calculates how much energy is assigned to each point in the `input`
    /// slice based on the specific distribution profile (e.g., Uniform or Gaussian).
    ///
    /// # Parameters
    /// - `input`: A slice of [`Point2<Length>`] representing the sampling coordinates.
    ///
    /// # Returns
    /// A [`Vec<Energy>`] containing the energy value for each corresponding input point.
    fn apply(&self, input: &[Point2<Length>]) -> Vec<Energy>;
    /// Returns the total integrated energy defined for this distribution.
    ///
    /// # Returns
    /// The total [`Energy`] value.
    fn get_total_energy(&self) -> Energy;
    /// Re-scales the provided energy vector to ensure energy conservation.
    ///
    /// This method adjusts the values in `energy_dist` so that their sum matches
    /// the value returned by [`Self::get_total_energy`]. It is typically used
    /// after filtering or clipping operations to restore the total energy budget.
    ///
    /// # Algorithm and Numerical Stability
    /// 1. A threshold `min_energy` is calculated as $E_{total} \times \epsilon$ (using [`f64::EPSILON`]).
    /// 2. Only energy values above this threshold are summed up to calculate the current total.
    /// 3. A scale factor is derived: $F = \frac{E_{target}}{E_{current\_valid}}$.
    /// 4. Every element in the vector is multiplied by this factor.
    ///
    /// # Parameters
    /// - `energy_dist`: A mutable reference to a vector of [`Energy`] values to be normalized in place.
    fn renormalize(&self, energy_dist: &mut Vec<Energy>) {
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

        if total_energy_valid_rays.value > 0.0 {
            let energy_scale_factor = self.get_total_energy() / total_energy_valid_rays;
            for e in energy_dist.iter_mut() {
                *e = *e * energy_scale_factor;
            }
        }
    }
}

impl Default for EnergyDistType {
    fn default() -> Self {
        Self::Uniform(UniformDist::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter, Copy, EnsureValidated)]
pub enum EnergyDistType {
    Uniform(UniformDist),
    General2DGaussian(general_gaussian::General2DGaussian),
}

impl DefaultFromName for EnergyDistType {}

impl EnergyDistType {
    /// Returns a reference to the internal energy distribution as a trait object.
    ///
    /// This allows polymorphic use of [`EnergyDistType`] through the [`EnergyDistribution`] trait.
    ///
    /// # Returns
    /// A reference to the [`dyn EnergyDistribution`] implementation stored in this enum.
    #[must_use]
    pub fn generate(&self) -> &dyn EnergyDistribution {
        match self {
            Self::Uniform(dist) => dist,
            Self::General2DGaussian(dist) => dist,
        }
    }

    /// Sets the total energy value for the current distribution.
    ///
    /// This method delegates to the corresponding `set_energy` method of the active variant.
    ///
    /// # Parameters
    /// - `energy`: The total [`Energy`] to be assigned.
    ///
    /// # Returns
    /// - `Ok(())` on success.
    /// - `Err(OpossumError)` if the assignment fails internally.
    ///
    /// # Errors
    /// Returns an error if the selected distribution variant rejects the energy value.
    ///
    pub fn set_energy(&mut self, energy: Energy) -> OpmResult<()> {
        match self {
            Self::Uniform(uniform_dist) => uniform_dist.set_energy(energy)?,
            Self::General2DGaussian(general2_dgaussian) => {
                general2_dgaussian.set_energy(energy)?;
            }
        }
        Ok(())
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
