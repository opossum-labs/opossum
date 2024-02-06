#![warn(missing_docs)]
//! Optical Analyzers
//!
//! An analyzer of a certain [`AnalyzerType`] determines how an (`OpticScenery`)[`crate::OpticScenery`] is analyzed. For example, the energy flow for a scenery can be
//! calculated as a simple analysis. On the other hand, a full Fourier propagation could be performed. The result of an analysis run can be written to a JSON structure
//! and / or exported as a PDF report.
use std::fmt::Display;
use strum::EnumIter;
use uom::si::{energy::picojoule, f64::Energy};

use crate::error::{OpmResult, OpossumError};

/// Type of analysis to be performed.
#[non_exhaustive]
#[derive(EnumIter, PartialEq, Debug)]
pub enum AnalyzerType {
    /// Simple energy flow of an optical spectrum.
    ///
    /// **Note**: This mode does consider any geometric aspects of an optical setup. In particular, possible apertures of optical elements are ignored.
    Energy,
    /// Ray tracing analysis.
    ///
    /// This mode simulates a bundle of optical ray propagating through a scenery.
    RayTrace(RayTraceConfig),
}

#[derive(Default, Debug, PartialEq, Copy, Clone)]
pub enum RayTracingMode {
    #[default]
    /// Sequential mode
    ///
    /// In this mode, rays follow the directed graph from node to node. If the next node is not hit, further propagation is dropped. This mode is
    /// mostly useful for imaging, collimation, and optimizing of "simple" optical lens systems.
    Sequential,
    // /// Semi-sequential mode
    // ///
    // /// Rays may bounce and traverse the graph in backward direction. If the next intended node is not hit, further propagation is dropped.
    // /// Interesting for ghost focus simulation
    // SemiSequential,
    // /// Non-sequential mode
    // ///
    // /// Rays do not follow a specific direction of the graph. Skipping of nodes may be allowed. Interesting for stray-light analysis, flash-lamp pumping, beam dumps, etc.
    // NonSequential
}

#[derive(PartialEq, Debug)]
/// Configuration data for a rays tracing analysis.
///
/// It currently only contains the `RayTracingMode`.
pub struct RayTraceConfig {
    mode: RayTracingMode,
    min_energy_per_ray: Energy,
}

impl RayTraceConfig {
    /// Returns the lower limit for ray energies during analysis. Rays with energies lower than this limit will be dropped.
    #[must_use]
    pub fn min_energy_per_ray(&self) -> Energy {
        self.min_energy_per_ray
    }

    /// Returns the ray-tracing mode of this config.
    pub fn mode(&self) -> RayTracingMode {
        self.mode
    }

    /// Sets the min energy per ray during analysis. Rays with energies lower than this limit will be dropped.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given energy limit is negative or not finite.
    pub fn set_min_energy_per_ray(&mut self, min_energy_per_ray: Energy) -> OpmResult<()> {
        if !min_energy_per_ray.is_finite() || min_energy_per_ray.is_sign_negative() {
            return Err(OpossumError::Analysis(
                "minimum energy must be >=0.0 and finite".into(),
            ));
        }
        self.min_energy_per_ray = min_energy_per_ray;
        Ok(())
    }
}
impl Default for RayTraceConfig {
    fn default() -> Self {
        Self {
            mode: RayTracingMode::default(),
            min_energy_per_ray: Energy::new::<picojoule>(1.0),
        }
    }
}
impl Display for AnalyzerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::Energy => "energy",
            Self::RayTrace(_) => "ray tracing",
        };
        write!(f, "{msg}")
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use assert_matches::assert_matches;
    #[test]
    fn display() {
        assert_eq!(format!("{}", AnalyzerType::Energy), "energy");
        assert_eq!(
            format!("{}", AnalyzerType::RayTrace(RayTraceConfig::default())),
            "ray tracing"
        );
    }
    #[test]
    fn debug() {
        assert_eq!(format!("{:?}", AnalyzerType::Energy), "Energy");
    }
    #[test]
    fn ray_tracing_mode_default() {
        assert_matches!(RayTracingMode::default(), RayTracingMode::Sequential);
    }
    #[test]
    fn ray_tracing_mode_debug() {
        assert_eq!(format!("{:?}", RayTracingMode::default()), "Sequential");
    }
    #[test]
    fn ray_tracing_config_default() {
        assert_matches!(RayTraceConfig::default().mode, RayTracingMode::Sequential);
    }
    #[test]
    fn ray_tracing_config_debug() {
        assert_eq!(
            format!("{:?}", RayTraceConfig::default()),
            "RayTraceConfig { mode: Sequential, min_energy_per_ray: 1e-12 m^2 kg^1 s^-2 }"
        );
    }
}
