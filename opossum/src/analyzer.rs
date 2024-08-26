#![warn(missing_docs)]
//! Optical Analyzers
//!
//! An analyzer of a certain [`AnalyzerType`] determines how an (`OpticScenery`)[`crate::OpticScenery`] is analyzed. For example, the energy flow for a scenery can be
//! calculated as a simple analysis. On the other hand, a full Fourier propagation could be performed. The result of an analysis run can be written to a JSON structure
//! and / or exported as a PDF report.
use std::fmt::Display;
use strum::EnumIter;
use uom::si::f64::Energy;

use crate::{
    error::{OpmResult, OpossumError},
    picojoule,
};

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
    /// Ghost focus analysis.
    ///
    /// This mode also performs ray tracing but considers parasitic back reflections from surfaces wtih a given number of bounces.
    GhostFocus(GhostFocusConfig),
}

/// enum to define the mode of the raytracing analysis.
/// Currently only sequential mode
#[derive(Default, Debug, PartialEq, Eq, Copy, Clone)]
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
/// The config contains the following info
///   - ray tracing mode (see [`RayTracingMode`])
///   - minimum energy / ray
///   - maximum number of bounces (reflections) / ray
///   - maximum number of refractions / ray
pub struct RayTraceConfig {
    mode: RayTracingMode,
    min_energy_per_ray: Energy,
    max_number_of_bounces: usize,
    max_number_of_refractions: usize,
}

impl RayTraceConfig {
    /// Returns the lower limit for ray energies during analysis. Rays with energies lower than this limit will be dropped.
    #[must_use]
    pub fn min_energy_per_ray(&self) -> Energy {
        self.min_energy_per_ray
    }

    /// Returns the ray-tracing mode of this config.
    #[must_use]
    pub const fn mode(&self) -> RayTracingMode {
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
    /// Returns the maximum number of bounces of this [`RayTraceConfig`].
    #[must_use]
    pub const fn max_number_of_bounces(&self) -> usize {
        self.max_number_of_bounces
    }
    /// Sets the max number of bounces of this [`RayTraceConfig`].
    pub fn set_max_number_of_bounces(&mut self, max_number_of_bounces: usize) {
        self.max_number_of_bounces = max_number_of_bounces;
    }
    /// Sets the max number of refractions of this [`RayTraceConfig`].
    pub fn set_max_number_of_refractions(&mut self, max_number_of_refractions: usize) {
        self.max_number_of_refractions = max_number_of_refractions;
    }
    /// Returns the max number of refractions of this [`RayTraceConfig`].
    #[must_use]
    pub const fn max_number_of_refractions(&self) -> usize {
        self.max_number_of_refractions
    }
}
impl Default for RayTraceConfig {
    /// Create a default config for a ray tracing analysis with the following parameters:
    ///   - ray tracing mode: [`RayTracingMode::Sequential`]
    ///   - mininum energy / ray: `1 p`
    ///   - maximum number of bounces / ray: `1000`
    ///   - maximum number od refractions / ray: `1000`
    fn default() -> Self {
        Self {
            mode: RayTracingMode::default(),
            min_energy_per_ray: picojoule!(1.0),
            max_number_of_bounces: 1000,
            max_number_of_refractions: 1000,
        }
    }
}

#[derive(PartialEq, Debug, Default)]
pub struct GhostFocusConfig;

impl Display for AnalyzerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::Energy => "energy",
            Self::RayTrace(_) => "ray tracing",
            Self::GhostFocus(_) => "ghost focus analysis",
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
        let rt_conf = RayTraceConfig::default();
        assert_matches!(rt_conf.mode, RayTracingMode::Sequential);
        assert_matches!(rt_conf.mode(), RayTracingMode::Sequential);
        assert_eq!(rt_conf.max_number_of_bounces, 1000);
        assert_eq!(rt_conf.max_number_of_bounces(), 1000);
        assert_eq!(rt_conf.max_number_of_refractions, 1000);
        assert_eq!(rt_conf.max_number_of_refractions(), 1000);
        assert_eq!(rt_conf.min_energy_per_ray, picojoule!(1.0));
        assert_eq!(rt_conf.min_energy_per_ray(), picojoule!(1.0));
    }
    #[test]
    fn ray_tracing_config_set_min_energy() {
        let mut rt_conf = RayTraceConfig::default();
        assert!(rt_conf.set_min_energy_per_ray(picojoule!(-0.1)).is_err());
        assert!(rt_conf
            .set_min_energy_per_ray(picojoule!(f64::NAN))
            .is_err());
        assert!(rt_conf
            .set_min_energy_per_ray(picojoule!(f64::INFINITY))
            .is_err());
        assert!(rt_conf.set_min_energy_per_ray(picojoule!(0.0)).is_ok());
        assert!(rt_conf.set_min_energy_per_ray(picojoule!(20.0)).is_ok());
        assert_eq!(rt_conf.min_energy_per_ray, picojoule!(20.0));
    }
    #[test]
    fn ray_tracing_config_setters() {
        let mut rt_conf = RayTraceConfig::default();
        rt_conf.set_max_number_of_bounces(123);
        rt_conf.set_max_number_of_refractions(456);
        assert_eq!(rt_conf.max_number_of_bounces, 123);
        assert_eq!(rt_conf.max_number_of_refractions, 456);
    }
    #[test]
    fn ray_tracing_config_debug() {
        assert_eq!(
            format!("{:?}", RayTraceConfig::default()),
            "RayTraceConfig { mode: Sequential, min_energy_per_ray: 1e-12 m^2 kg^1 s^-2, max_number_of_bounces: 1000, max_number_of_refractions: 1000 }"
        );
    }
}
