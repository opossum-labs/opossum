use std::fmt::Display;

use serde::{Deserialize, Serialize};
use uom::si::{energy::picojoule, f64::Energy};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum AnalyzerType {
    /// Simple energy flow analysis of an optical spectrum.
    ///
    /// **Note**: This mode does not consider any geometric aspects of an optical setup (so far). In particular,
    /// possible apertures of optical elements are ignored.
    Energy,
    /// Ray tracing analysis.
    ///
    /// This mode simulates a bundle of optical ray propagating through a scenery.
    RayTrace(RayTraceConfig),
    /// Ghost focus analysis.
    ///
    /// This mode also performs ray tracing but considers parasitic back reflections from surfaces wtih a
    /// given number of bounces.
    GhostFocus(GhostFocusConfig),
}
impl Display for AnalyzerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::Energy => "energy",
            Self::RayTrace(_) => "ray tracing",
            Self::GhostFocus(_) => "ghost focus",
        };
        write!(f, "{msg} analysis")
    }
}

/// Strategy to use if a [`Ray`](crate::ray::Ray) misses a surface
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum MissedSurfaceStrategy {
    /// The [`Ray`](crate::ray::Ray) it is set as invalid and does no longer propagate.
    Stop,
    /// The [`Ray`](crate::ray::Ray) is not altered in any way, thus skipping the surface and propagating
    /// further through the system.
    Ignore,
}
impl Default for MissedSurfaceStrategy {
    fn default() -> Self {
        Self::Stop
    }
}
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
/// Configuration data for a rays tracing analysis.
///
/// The config contains the following info
///   - minimum energy / ray
///   - maximum number of bounces (reflections) / ray
///   - maximum number of refractions / ray
pub struct RayTraceConfig {
    min_energy_per_ray: Energy,
    max_number_of_bounces: usize,
    max_number_of_refractions: usize,
    missed_surface_strategy: MissedSurfaceStrategy,
}
impl Default for RayTraceConfig {
    /// Create a default config for a ray tracing analysis with the following parameters:
    ///   - mininum energy / ray: `1 pJ`
    ///   - maximum number of bounces / ray: `1000`
    ///   - maximum number of refractions / ray: `1000`
    ///   - missed surface strategy: ray is stopped
    fn default() -> Self {
        Self {
            min_energy_per_ray: Energy::new::<picojoule>(1.0),
            max_number_of_bounces: 1000,
            max_number_of_refractions: 1000,
            missed_surface_strategy: MissedSurfaceStrategy::default(),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
/// Configuration for performing a ghost focus analysis
pub struct GhostFocusConfig {
    max_bounces: usize,
    fluence_estimator: FluenceEstimator,
}
impl Default for GhostFocusConfig {
    fn default() -> Self {
        Self {
            max_bounces: 1,
            fluence_estimator: FluenceEstimator::Voronoi,
        }
    }
}
// Strategy for fluence estimation
#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FluenceEstimator {
    /// Calculate Voronoi cells of the hit points and use the cell area for calculation of the fluence.
    #[default]
    Voronoi,
    /// Calculate the fluence at given point using a Kernel Density Estimator
    KDE,
    /// Simply perform binning of the hit points on a given matrix (not implemented yet)
    Binning,
    /// Using additional "helper rays" for each ray to calculate the evolution of a small area element around the intial ray to calcuklate the fluence
    HelperRays,
}
