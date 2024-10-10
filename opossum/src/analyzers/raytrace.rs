//! Analyzer for sequential ray tracing
use super::Analyzer;
use crate::{
    error::{OpmResult, OpossumError},
    light_result::LightResult,
    nodes::NodeGroup,
    optic_node::OpticNode,
    picojoule,
    reporting::analysis_report::AnalysisReport,
};
use log::info;
use serde::{Deserialize, Serialize};
use uom::si::f64::Energy;

//pub type LightResRays = LightDings<Rays>;

/// Analyzer for (sequential) ray tracing
#[derive(Default, Debug)]
pub struct RayTracingAnalyzer {
    config: RayTraceConfig,
}
impl RayTracingAnalyzer {
    /// Creates a new [`RayTracingAnalyzer`].
    #[must_use]
    pub const fn new(config: RayTraceConfig) -> Self {
        Self { config }
    }
}
impl Analyzer for RayTracingAnalyzer {
    fn analyze(&self, scenery: &mut NodeGroup) -> OpmResult<()> {
        let scenery_name = if scenery.node_attr().name().is_empty() {
            String::new()
        } else {
            format!(" '{}'", scenery.node_attr().name())
        };
        info!("Calculate node positions of scenery{scenery_name}.");
        AnalysisRayTrace::calc_node_position(scenery, LightResult::default(), &self.config)?;
        scenery.reset_data();
        info!("Performing ray tracing analysis of scenery{scenery_name}.");
        AnalysisRayTrace::analyze(scenery, LightResult::default(), &self.config)?;
        Ok(())
    }
    fn report(&self, scenery: &NodeGroup) -> OpmResult<AnalysisReport> {
        scenery.toplevel_report()
    }
}
/// Trait for implementing the ray trace analysis.
pub trait AnalysisRayTrace: OpticNode {
    /// Perform a ray trace analysis an [`OpticNode`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult>;
    /// Calculate the position of this [`OpticNode`] element.
    ///
    /// This function calculates the position of this [`OpticNode`] element in 3D space. This is based on the analysis of a single,
    /// central [`Ray`](crate::ray::Ray) representing the optical axis. The default implementation is to use the normal `analyze`
    /// function. For a [`NodeGroup`] however, this must be separately implemented in order to allow nesting.
    ///
    /// # Errors
    /// This function will return an error if internal element-specific errors occur and the analysis cannot be performed.
    fn calc_node_position(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        self.analyze(incoming_data, config)
    }
}
// /// enum to define the mode of the raytracing analysis.
// /// Currently only sequential mode
// #[derive(Default, Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
// pub enum RayTracingMode {
//     #[default]
//     /// Sequential mode
//     ///
//     /// In this mode, rays follow the directed graph from node to node. If the next node is not hit, further propagation is dropped. This mode is
//     /// mostly useful for imaging, collimation, and optimizing of "simple" optical lens systems.
//     Sequential,
//     // /// Semi-sequential mode
//     // ///
//     // /// Rays may bounce and traverse the graph in backward direction. If the next intended node is not hit, further propagation is dropped.
//     // /// Interesting for ghost focus simulation
//     // SemiSequential,
//     // /// Non-sequential mode
//     // ///
//     // /// Rays do not follow a specific direction of the graph. Skipping of nodes may be allowed. Interesting for stray-light analysis, flash-lamp pumping, beam dumps, etc.
//     // NonSequential
// }

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
/// Configuration data for a rays tracing analysis.
///
/// The config contains the following info
///   - ray tracing mode (see [`RayTracingMode`])
///   - minimum energy / ray
///   - maximum number of bounces (reflections) / ray
///   - maximum number of refractions / ray
pub struct RayTraceConfig {
    //mode: RayTracingMode,
    min_energy_per_ray: Energy,
    max_number_of_bounces: usize,
    max_number_of_refractions: usize,
}
impl Default for RayTraceConfig {
    /// Create a default config for a ray tracing analysis with the following parameters:
    ///   - ray tracing mode: [`RayTracingMode::Sequential`]
    ///   - mininum energy / ray: `1 p`
    ///   - maximum number of bounces / ray: `1000`
    ///   - maximum number od refractions / ray: `1000`
    fn default() -> Self {
        Self {
            //mode: RayTracingMode::default(),
            min_energy_per_ray: picojoule!(1.0),
            max_number_of_bounces: 1000,
            max_number_of_refractions: 1000,
        }
    }
}
impl RayTraceConfig {
    /// Returns the lower limit for ray energies during analysis. Rays with energies lower than this limit will be dropped.
    #[must_use]
    pub fn min_energy_per_ray(&self) -> Energy {
        self.min_energy_per_ray
    }

    /// Returns the ray-tracing mode of this config.
    // #[must_use]
    // pub const fn mode(&self) -> RayTracingMode {
    //     self.mode
    // }
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        joule, millimeter,
        nodes::{round_collimated_ray_source, ParaxialSurface},
    };
    #[test]
    fn ray_tracing_config_default() {
        let rt_conf = RayTraceConfig::default();
        // assert!(matches!(rt_conf.mode(), RayTracingMode::Sequential));
        assert_eq!(rt_conf.max_number_of_bounces(), 1000);
        assert_eq!(rt_conf.max_number_of_refractions(), 1000);
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
            "RayTraceConfig { min_energy_per_ray: 1e-12 m^2 kg^1 s^-2, max_number_of_bounces: 1000, max_number_of_refractions: 1000 }"
        );
    }
    #[test]
    fn ray_tracing_integration_test() {
        // simulate simple system for integration test
        let mut group = NodeGroup::default();
        let i_src = group
            .add_node(round_collimated_ray_source(millimeter!(10.0), joule!(1.0), 3).unwrap())
            .unwrap();
        let i_l1 = group
            .add_node(ParaxialSurface::new("f=100", millimeter!(100.0)).unwrap())
            .unwrap();
        group
            .connect_nodes(i_src, "out1", i_l1, "front", millimeter!(50.0))
            .unwrap();
        let analyzer = RayTracingAnalyzer::default();
        analyzer.analyze(&mut group).unwrap();
    }
}
