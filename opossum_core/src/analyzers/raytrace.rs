#![warn(missing_docs)]
//! Analyzer for sequential ray tracing
use std::collections::HashMap;

use super::{Analyzer, AnalyzerType};
use crate::{
    analyzers::propagation_strategy::{MissedSurfaceStrategy, PropagationStrategy},
    core_optics::{NodeAttrExt, OpticNode, OpticNodeExt, node_attr::HasNodeAttr},
    error::{OpmResult, OpossumError},
    gain::{ActiveScenario, PumpConfig, PumpScenario},
    light::{LightResult, Rays, lightdata::ray_data_builder::RayDataBuilder},
    nodes::NodeGroup,
    picojoule,
    reporting::analysis_report::AnalysisReport,
};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AnalyzerRegistration;

inventory::submit! {
    AnalyzerRegistration::new(
        || AnalyzerType::RayTrace(RayTraceConfig::default()),
        |at| if let AnalyzerType::RayTrace(config) = at { Some(Box::new(RayTracingAnalyzer::new(config.clone()))) } else { None }
    )
}
use uom::si::f64::Energy;

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
/// Configuration data for a rays tracing analysis.
///
/// The config contains the following info
///  - minimum energy / ray
///  - maximum number of bounces (reflections) / ray
///  - maximum number of refractions / ray
///  - map of `SourcePort` nodes to their respective light definition
pub struct RayTraceConfig {
    min_energy_per_ray: Energy,
    max_number_of_bounces: usize,
    max_number_of_refractions: usize,
    missed_surface_strategy: MissedSurfaceStrategy,
    source_map: HashMap<Uuid, RayDataBuilder>,
    /// The operating point of the run currently being performed - see [`ActiveScenario`]. Not part
    /// of the configuration a user edits and not written to file.
    #[serde(skip)]
    active_pump_scenario: ActiveScenario,
}
impl Default for RayTraceConfig {
    /// Create a default config for a ray tracing analysis with the following parameters:
    ///  - mininum energy / ray: `1 pJ`
    ///  - maximum number of bounces / ray: `1000`
    ///  - maximum number of refractions / ray: `1000`
    ///  - missed surface strategy: ray is stopped
    ///  - empty source map
    fn default() -> Self {
        Self {
            min_energy_per_ray: picojoule!(1.0),
            max_number_of_bounces: 1000,
            max_number_of_refractions: 1000,
            missed_surface_strategy: MissedSurfaceStrategy::Stop,
            source_map: HashMap::new(),
            active_pump_scenario: ActiveScenario::default(),
        }
    }
}
impl RayTraceConfig {
    /// Returns the lower limit for ray energies during analysis. Rays with energies lower than this limit will be dropped.
    #[must_use]
    pub fn min_energy_per_ray(&self) -> Energy {
        self.min_energy_per_ray
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
    pub const fn set_max_number_of_bounces(&mut self, max_number_of_bounces: usize) {
        self.max_number_of_bounces = max_number_of_bounces;
    }
    /// Sets the max number of refractions of this [`RayTraceConfig`].
    pub const fn set_max_number_of_refractions(&mut self, max_number_of_refractions: usize) {
        self.max_number_of_refractions = max_number_of_refractions;
    }
    /// Returns the max number of refractions of this [`RayTraceConfig`].
    #[must_use]
    pub const fn max_number_of_refractions(&self) -> usize {
        self.max_number_of_refractions
    }
    /// Returns a reference to the `missed surface strategy` of this [`RayTraceConfig`].
    #[must_use]
    pub const fn missed_surface_strategy(&self) -> &MissedSurfaceStrategy {
        &self.missed_surface_strategy
    }
    /// Sets the `missed surface strategy` of this [`RayTraceConfig`].
    pub const fn set_missed_surface_strategy(
        &mut self,
        missed_surface_strategy: MissedSurfaceStrategy,
    ) {
        self.missed_surface_strategy = missed_surface_strategy;
    }
    /// Maps a source UUID to a ray data builder.
    ///
    /// If a builder was already mapped this function returns `true`. A new mapping
    /// reutrns `false`
    pub fn map_source(&mut self, uuid: Uuid, builder: RayDataBuilder) -> bool {
        self.source_map.insert(uuid, builder).is_some()
    }
    /// Returns a reference to the ray data builder mapped to the given source UUID, if any.
    #[must_use]
    pub fn get_source(&self, uuid: &Uuid) -> Option<&RayDataBuilder> {
        self.source_map.get(uuid)
    }
    /// Removes and returns the ray data builder mapped to the given source UUID, if any.
    #[must_use]
    pub fn remove_source(&mut self, uuid: &Uuid) -> Option<RayDataBuilder> {
        self.source_map.remove(uuid)
    }
    /// Set the [`PumpScenario`] this analysis run is being performed in.
    ///
    /// # Arguments
    ///
    /// * `pump_scenario` - the operating point, or `None` for a passive run.
    pub fn set_active_pump_scenario(&mut self, pump_scenario: Option<PumpScenario>) {
        self.active_pump_scenario.set(pump_scenario);
    }
    /// Removes all source mappings whose UUIDs no longer exist in the given model.
    pub fn prune_source_map(&mut self, model: &NodeGroup) {
        self.source_map.retain(|uuid, _builder| model.exists(*uuid));
    }
    /// The first source UUID whose mapping differs from `other`'s (added, removed, or changed value), if
    /// any. Used to focus the exact source-port card an undo/redo of a source-mapping change touched.
    #[must_use]
    pub fn first_differing_source(&self, other: &Self) -> Option<Uuid> {
        self.source_map
            .keys()
            .chain(other.source_map.keys())
            .copied()
            .find(|uuid| self.source_map.get(uuid) != other.source_map.get(uuid))
    }
    /// Sets the entire source map of this [`RayTraceConfig`].
    ///
    /// This will overwrite any existing mappings. Use with care.
    pub fn set_source_map(&mut self, source_map: HashMap<Uuid, RayDataBuilder>) {
        self.source_map = source_map;
    }
}

impl PropagationStrategy for RayTraceConfig {
    fn missed_surface_strategy(&self) -> MissedSurfaceStrategy {
        *self.missed_surface_strategy()
    }
    fn pump_config(&self, node_id: Uuid) -> PumpConfig {
        self.active_pump_scenario.config(node_id)
    }
    fn on_after_apodization(&self, rays: &mut Rays) -> OpmResult<()> {
        rays.invalidate_by_threshold_energy(self.min_energy_per_ray())?;
        Ok(())
    }
}
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
        AnalysisRayTrace::calc_node_positions(scenery, LightResult::default(), &self.config)?;
        scenery.reset_data();
        scenery.prepare_volume(&self.config)?;
        info!("Performing ray tracing analysis of scenery{scenery_name}.");
        AnalysisRayTrace::analyze(scenery, LightResult::default(), &self.config)?;
        Ok(())
    }
    fn report(&self, scenery: &NodeGroup) -> OpmResult<AnalysisReport> {
        let mut report = scenery.toplevel_report()?;
        report.set_analysis_type("Ray Tracing Analysis");
        Ok(report)
    }
}
/// Trait for implementing the ray trace analysis.
pub trait AnalysisRayTrace: OpticNode {
    /// Perform a ray trace analysis an [`OpticNode`].
    ///
    /// # Errors
    ///
    /// This function will return an error if internal element-specific errors occur and the analysis cannot be performed.
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        self.unified_analyze_single_surface_node(incoming_data, config, "input_1", None)
    }
    /// Calculate the position of this [`OpticNode`] element.
    ///
    /// This function calculates the position of this [`OpticNode`] element in 3D space. This is based on the analysis of a single,
    /// central [`Ray`](crate::light::ray::Ray) representing the optical axis. The default implementation is to use the normal `analyze`
    /// function. For a [`NodeGroup`] however, this must be separately implemented in order to allow nesting.
    ///
    /// # Errors
    /// This function will return an error if internal element-specific errors occur and the analysis cannot be performed.
    fn calc_node_positions(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        if incoming_data.is_empty() {
            warn!(
                "{} got no valid optical axis data from previous node and can thus not being placed. Skipping.",
                self.node_info()
            );
            Ok(LightResult::default())
        } else {
            self.analyze(incoming_data, config)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        joule, millimeter,
        nodes::{Dummy, ParaxialSurface, SourcePort, round_collimated_ray_builder},
        utils::test_helper::test_helper::check_logs,
    };
    #[test]
    fn config_default() {
        let rt_conf = RayTraceConfig::default();
        assert_eq!(rt_conf.max_number_of_bounces(), 1000);
        assert_eq!(rt_conf.max_number_of_refractions(), 1000);
        assert_eq!(rt_conf.min_energy_per_ray(), picojoule!(1.0));
    }
    #[test]
    fn config_set_min_energy() {
        let mut rt_conf = RayTraceConfig::default();
        assert!(rt_conf.set_min_energy_per_ray(picojoule!(-0.1)).is_err());
        assert!(
            rt_conf
                .set_min_energy_per_ray(picojoule!(f64::NAN))
                .is_err()
        );
        assert!(
            rt_conf
                .set_min_energy_per_ray(picojoule!(f64::INFINITY))
                .is_err()
        );
        assert!(rt_conf.set_min_energy_per_ray(picojoule!(0.0)).is_ok());
        assert!(rt_conf.set_min_energy_per_ray(picojoule!(20.0)).is_ok());
        assert_eq!(rt_conf.min_energy_per_ray, picojoule!(20.0));
    }
    #[test]
    fn config_setters() {
        let mut rt_conf = RayTraceConfig::default();
        rt_conf.set_max_number_of_bounces(123);
        rt_conf.set_max_number_of_refractions(456);
        assert_eq!(rt_conf.max_number_of_bounces, 123);
        assert_eq!(rt_conf.max_number_of_refractions, 456);
    }
    #[test]
    fn config_debug() {
        assert_eq!(
            format!("{:?}", RayTraceConfig::default()),
            "RayTraceConfig { min_energy_per_ray: 1e-12 m^2 kg^1 s^-2, max_number_of_bounces: 1000, max_number_of_refractions: 1000, missed_surface_strategy: Stop, source_map: {}, active_pump_scenario: ActiveScenario(None) }"
        );
    }
    #[test]
    fn new() {
        let mut config = RayTraceConfig::default();
        config.set_max_number_of_bounces(123);
        let analyzer = RayTracingAnalyzer::new(config);
        assert_eq!(analyzer.config.max_number_of_bounces(), 123);
    }
    #[test]
    fn analyze_info() -> OpmResult<()> {
        let mut scenery = NodeGroup::new("test");
        let analyzer = RayTracingAnalyzer::default();
        testing_logger::setup();
        analyzer.analyze(&mut scenery)?;
        check_logs(
            log::Level::Info,
            vec![
                "Calculate node positions of scenery 'test'.",
                "Performing ray tracing analysis of scenery 'test'.",
            ],
        );
        let mut scenery = NodeGroup::new("");
        let analyzer = RayTracingAnalyzer::default();
        testing_logger::setup();
        analyzer.analyze(&mut scenery)?;
        check_logs(
            log::Level::Info,
            vec![
                "Calculate node positions of scenery.",
                "Performing ray tracing analysis of scenery.",
            ],
        );
        Ok(())
    }
    #[test]
    fn report() -> OpmResult<()> {
        let analyzer = RayTracingAnalyzer::default();
        let scenery = NodeGroup::new("");
        assert!(analyzer.report(&scenery).is_ok());
        Ok(())
    }
    #[test]
    #[ignore]
    fn integration_test() -> OpmResult<()> {
        // simulate simple system for integration test
        let mut group = NodeGroup::default();
        let i_src = group.add_node(SourcePort::default())?;
        let i_l1 = group.add_node(ParaxialSurface::new("f=100", millimeter!(100.0))?)?;
        group.connect_nodes(i_src, "output_1", i_l1, "input_1", millimeter!(50.0))?;
        let mut config = RayTraceConfig::default();
        config.map_source(
            i_src,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 3)?,
        );
        let analyzer = RayTracingAnalyzer::new(config);
        assert!(analyzer.analyze(&mut group).is_ok());
        Ok(())
    }
    #[test]
    fn test_position_history_integration() -> OpmResult<()> {
        use crate::{
            analyzers::{Analyzer, raytrace::RayTracingAnalyzer},
            nodes::{ParaxialSurface, RayPropagationVisualizer},
            properties::Proptype,
            utils::lock_ext::LockExt,
        };
        // Source -> Lens -> Visualizer
        let mut group = NodeGroup::default();
        let i_src = group.add_node(SourcePort::default())?;
        let i_lens = group.add_node(ParaxialSurface::new("lens", millimeter!(100.0))?)?;
        let i_det = group.add_node(RayPropagationVisualizer::default())?;
        group.connect_nodes(i_src, "output_1", i_lens, "input_1", millimeter!(50.0))?;
        group.connect_nodes(i_lens, "output_1", i_det, "input_1", millimeter!(50.0))?;
        let mut config = RayTraceConfig::default();
        config.map_source(
            i_src,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 3)?,
        );
        let analyzer = RayTracingAnalyzer::new(config);
        assert!(analyzer.analyze(&mut group).is_ok());
        let node_ref = group.graph().node(i_det)?;
        let det_node = node_ref.optical_ref.lock_opm()?;
        let report = det_node
            .node_report("test_uuid")?
            .ok_or_else(|| OpossumError::Other("got empty report".into()))?;
        let prop = report.properties().get("Ray plot")?;
        if let Proptype::RayPositionHistory(hist) = prop {
            assert!(
                !hist.rays_pos_history.is_empty(),
                "No spectral history buckets found"
            );
            let ray_histories = hist.rays_pos_history[0].get_history();
            assert!(!ray_histories.is_empty(), "No rays found in history bucket");

            let first_ray_hist = &ray_histories[0];
            let num_points = first_ray_hist.nrows();
            assert!(
                num_points >= 3,
                "Ray history is broken! Expected at least 3 points (Source, Lens, Detector), but got only {}",
                num_points
            );
        } else {
            panic!("'Ray plot' property has the wrong Proptype!");
        }
        Ok(())
    }
    #[test]
    fn test_map_and_get_source() {
        use crate::light::lightdata::ray_data_source::{CollimatedSrc, PointSrc, RayDataSource};
        use uuid::Uuid;
        let mut config = RayTraceConfig::default();
        let uuid = Uuid::new_v4();
        let source = RayDataSource::Collimated(CollimatedSrc::default());

        assert_eq!(config.map_source(uuid, source.clone().into()), false);
        assert_eq!(config.get_source(&uuid), Some(&source.clone().into()));

        // Let's use PointSrc for the second one to be sure it's different
        let source2 = RayDataSource::PointSrc(PointSrc::default());

        assert_eq!(config.map_source(uuid, source2.clone().into()), true);
        assert_eq!(config.get_source(&uuid), Some(&source2.clone().into()));
    }

    #[test]
    fn test_remove_source() {
        use crate::light::lightdata::ray_data_source::{CollimatedSrc, RayDataSource};
        use uuid::Uuid;
        let mut config = RayTraceConfig::default();
        let uuid = Uuid::new_v4();
        let source = RayDataSource::Collimated(CollimatedSrc::default());

        config.map_source(uuid, source.clone().into());
        assert_eq!(config.remove_source(&uuid), Some(source.into()));
        assert!(config.get_source(&uuid).is_none());
        assert!(config.remove_source(&uuid).is_none());
    }

    #[test]
    fn test_prune_source_map() -> OpmResult<()> {
        use crate::light::lightdata::ray_data_source::{CollimatedSrc, RayDataSource};
        use uuid::Uuid;

        let mut scene = NodeGroup::default();
        let src = SourcePort::default();
        let node_id = scene.add_node(src)?;

        let mut config = RayTraceConfig::default();
        let source = RayDataSource::Collimated(CollimatedSrc::default());
        config.map_source(node_id, source.clone().into());

        let uuid2 = Uuid::new_v4();
        config.map_source(uuid2, source.into());

        config.prune_source_map(&scene);

        assert!(config.get_source(&node_id).is_some());
        assert!(config.get_source(&uuid2).is_none());
        Ok(())
    }
    #[test]
    fn test_no_optical_axis_warning() -> OpmResult<()> {
        let mut scenery = NodeGroup::new("OpticScenery demo");
        let node1 = scenery.add_node(Dummy::new("dummy1"))?;
        let node2 = scenery.add_node(Dummy::new("dummy2"))?;
        scenery.connect_nodes(node1, "output_1", node2, "input_1", millimeter!(0.0))?;
        let analyzer = RayTracingAnalyzer::default();
        testing_logger::setup();
        analyzer.analyze(&mut scenery)?;
        check_logs(
            log::Level::Warn,
            vec![
                "'dummy1' (dummy) has no incoming connections and can thus not being placed. Skipping.",
                "'dummy2' (dummy) got no valid optical axis data from previous node and can thus not being placed. Skipping.",
            ],
        );
        Ok(())
    }
}
