#![warn(missing_docs)]
//! Analyzer for sequential ray tracing
use std::collections::BTreeMap;

use super::AnalyzerRegistration;
use super::{Analyzer, AnalyzerType};
use crate::{
    analyzers::propagation_strategy::{MissedSurfaceStrategy, PropagationStrategy},
    core_optics::{OpticNode, OpticNodeExt, node_attr::HasNodeAttr, optic_surface::OpticSurface},
    error::{OpmResult, OpossumError},
    gain::{ActiveScenario, PumpConfig, PumpScenario},
    light::{LightResult, Rays, lightdata::ray_data_builder::RayDataBuilder},
    material::Material,
    nodes::NodeGroup,
    picojoule,
    properties::proptype::AssetRef,
    reporting::analysis_report::AnalysisReport,
    utils::geom_transformation::Isometry,
};
use log::info;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uom::si::f64::Energy;
use uuid::Uuid;

inventory::submit! {
    AnalyzerRegistration::new(
        || AnalyzerType::RayTrace(RayTraceConfig::default()),
        |at| if let AnalyzerType::RayTrace(config) = at { Some(Box::new(RayTracingAnalyzer::new(config.clone()))) } else { None }
    )
}

/// The ambient medium of an analysis or a setup that names none: vacuum.
pub(crate) fn default_ambient_material() -> AssetRef<Material> {
    AssetRef::Inline(Material::vacuum())
}

/// Whether `asset` is the default ambient medium, vacuum, which is not written to file.
pub(crate) const fn is_default_vacuum(asset: &AssetRef<Material>) -> bool {
    match asset {
        AssetRef::Inline(mat) => mat.id().is_nil(),
        AssetRef::Id(id) => id.is_nil(),
    }
}

/// Configuration data for a rays tracing analysis.
///
/// The config contains the following info
///  - minimum energy / ray
///  - maximum number of bounces (reflections) / ray
///  - maximum number of refractions / ray
///  - map of `SourcePort` nodes to their respective light definition
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RayTraceConfig {
    #[schemars(with = "f64")]
    min_energy_per_ray: Energy,
    max_number_of_bounces: usize,
    max_number_of_refractions: usize,
    missed_surface_strategy: MissedSurfaceStrategy,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    source_map: BTreeMap<Uuid, RayDataBuilder>,
    #[serde(
        default = "default_ambient_material",
        skip_serializing_if = "is_default_vacuum"
    )]
    ambient_material: AssetRef<Material>,
    #[serde(skip)]
    active_pump_scenario: ActiveScenario,
    #[serde(skip)]
    positioning_run: bool,
    #[serde(skip)]
    collect_ray_ends: bool,
    /// What the light of the run is traced for, which decides how much its sources prepare it. Set
    /// for the run by [`RayTracingAnalyzer::trace_positioned`]; not saved to file.
    #[serde(skip)]
    purpose: TracePurpose,
}
/// What the light of a ray trace is traced for, which decides how much its sources prepare it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TracePurpose {
    /// An analysis: the sources spread the energy they sample over the area each ray stands for,
    /// and span ray tubes if a fluence map follows them.
    Analysis {
        /// Whether the sources span ray tubes (see [`RayMesh`](crate::light::RayMesh)).
        ray_tubes: bool,
    },
    /// Drawing the light, as the 3D view does: the sources send it out as they build it.
    #[default]
    Drawing,
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
            source_map: BTreeMap::new(),
            ambient_material: AssetRef::Inline(Material::vacuum()),
            active_pump_scenario: ActiveScenario::default(),
            positioning_run: false,
            collect_ray_ends: false,
            purpose: TracePurpose::Drawing,
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
    /// Returns the ray data builders of all sources, keyed by the UUID of their source port.
    #[must_use]
    pub const fn source_map(&self) -> &BTreeMap<Uuid, RayDataBuilder> {
        &self.source_map
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
    pub fn set_source_map(&mut self, source_map: BTreeMap<Uuid, RayDataBuilder>) {
        self.source_map = source_map;
    }
    /// Returns a reference to the ambient material of this analysis.
    #[must_use]
    pub fn ambient_material(&self) -> &Material {
        self.ambient_material.unwrap_inline()
    }

    /// Sets the ambient material for this analysis.
    pub fn set_ambient_material(&mut self, material: Material) {
        self.ambient_material = AssetRef::Inline(material);
    }

    /// Returns a reference to the raw [`AssetRef`] for serialization/resolution.
    #[must_use]
    pub const fn ambient_material_ref(&self) -> &AssetRef<Material> {
        &self.ambient_material
    }

    /// Sets the raw [`AssetRef`] for serialization/resolution.
    pub fn set_ambient_material_ref(&mut self, asset: AssetRef<Material>) {
        self.ambient_material = asset;
    }
    /// Mark this config as driving the geometry-positioning run.
    ///
    /// When set, [`PropagationStrategy::is_positioning_run`] returns `true` and gain models skip
    /// amplification, because [`OpticNode::prepare_volume`] has not been called yet.
    pub const fn set_positioning_run(&mut self, positioning_run: bool) {
        self.positioning_run = positioning_run;
    }
    /// Sets whether [`NodeGroup`] instances should collect ray bundles that exit through
    /// unconnected, unmapped output ports.
    ///
    /// When set to `true`, every walk run with this config appends those bundles to
    /// [`NodeGroup::ray_ends`]; nothing clears them. Set it only on a fresh copy of a model (e.g.
    /// one created from its file string), which starts with no ray ends. When `false` (the
    /// default) the bundles are discarded and no extra memory is allocated — normal simulations
    /// are completely unaffected.
    pub const fn set_collect_ray_ends(&mut self, collect: bool) {
        self.collect_ray_ends = collect;
    }
    /// Returns whether ray bundles at unconnected, unmapped output ports are collected.
    ///
    /// See [`set_collect_ray_ends`](Self::set_collect_ray_ends) for details.
    #[must_use]
    pub const fn collect_ray_ends(&self) -> bool {
        self.collect_ray_ends
    }
    /// Return a config suitable for the geometry-positioning run.
    ///
    /// Copies the source map and scalar settings from `self` but drops the `active_pump_scenario`
    /// (to avoid cloning a potentially large [`PumpScenario`]) and sets `positioning_run = true`
    /// so that gain models know no medium has been prepared yet.  `collect_ray_ends` is
    /// explicitly set to `false`: a positioning run must not also collect the ray ends of the
    /// analysis walk it precedes.
    ///
    /// A missed surface always stops the ray, whatever `self` says: the optical axis has to pass
    /// every component it is connected through, and positioning stops where it does not.
    #[must_use]
    pub fn for_positioning(&self) -> Self {
        Self {
            min_energy_per_ray: self.min_energy_per_ray,
            max_number_of_bounces: self.max_number_of_bounces,
            max_number_of_refractions: self.max_number_of_refractions,
            missed_surface_strategy: MissedSurfaceStrategy::Stop,
            source_map: self.source_map.clone(),
            active_pump_scenario: ActiveScenario::default(),
            ambient_material: self.ambient_material_ref().clone(),
            positioning_run: true,
            collect_ray_ends: false,
            purpose: TracePurpose::Drawing,
        }
    }
}

impl PropagationStrategy for RayTraceConfig {
    fn missed_surface_strategy(&self) -> MissedSurfaceStrategy {
        *self.missed_surface_strategy()
    }
    fn ambient_refractive_index(&self) -> crate::refractive_index::RefractiveIndexType {
        self.ambient_material().refractive_index_type().clone()
    }
    fn pump_config(&self, node_id: Uuid) -> PumpConfig {
        self.active_pump_scenario.config(node_id)
    }
    fn is_positioning_run(&self) -> bool {
        self.positioning_run
    }
    /// Set for the run by [`RayTracingAnalyzer::trace_positioned`]; positioning never spans them.
    fn needs_ray_tubes(&self) -> bool {
        matches!(self.purpose, TracePurpose::Analysis { ray_tubes: true })
    }
    /// Set for the run by [`RayTracingAnalyzer::trace_positioned`]: in an analysis, not when the
    /// light is drawn or the nodes are positioned.
    fn spreads_energy_over_cells(&self) -> bool {
        matches!(self.purpose, TracePurpose::Analysis { .. })
    }
    fn on_surface_interaction(
        &self,
        _surf: &mut OpticSurface,
        rays: &mut Rays,
        _reflected_rays: Rays,
        _backward: bool,
    ) -> OpmResult<()> {
        // The optical axis is placed by geometry alone; its energy does not decide where it runs.
        if self.positioning_run {
            return Ok(());
        }
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
    /// Traces the light of the sources through a scenery whose nodes are positioned already,
    /// without positioning them again, as [`analyze`](Analyzer::analyze) does for an analysis.
    ///
    /// A copy positioned by
    /// [`OpmDocument::run_positioning`](crate::opm_document::OpmDocument::run_positioning) is
    /// traced where its nodes stand.
    ///
    /// # Arguments
    ///
    /// - `scenery`: the positioned scenery to trace, which keeps the light it receives
    /// - `purpose`: what the light is traced for: an analysis, whose sources span ray tubes if
    ///   [`OpticNode::needs_ray_tubes`] of the scenery says so, or drawing the light
    ///
    /// # Errors
    ///
    /// This function returns an error if a node cannot prepare its medium or the trace fails.
    pub fn trace_positioned(
        &self,
        scenery: &mut NodeGroup,
        purpose: TracePurpose,
    ) -> OpmResult<()> {
        let config = RayTraceConfig {
            purpose,
            ..self.config.clone()
        };
        scenery.reset_data();
        scenery.prepare_volume(&config)?;
        AnalysisRayTrace::analyze(scenery, LightResult::default(), &config)?;
        Ok(())
    }
}
impl Analyzer for RayTracingAnalyzer {
    fn analyze(&self, scenery: &mut NodeGroup) -> OpmResult<()> {
        let scenery_name = if scenery.node_attr().name().is_empty() {
            String::new()
        } else {
            format!(" '{}'", scenery.node_attr().name())
        };

        info!("Performing ray tracing analysis of scenery{scenery_name}.");
        let ray_tubes = scenery.needs_ray_tubes();
        self.trace_positioned(scenery, TracePurpose::Analysis { ray_tubes })
    }
    fn report(&self, scenery: &NodeGroup) -> OpmResult<AnalysisReport> {
        let mut report = scenery.toplevel_report(super::AnalyzerKind::RayTrace)?;
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
        self.unified_analyze_single_surface_node(incoming_data, config, "input_1")
    }
    /// Calculate the position of this [`OpticNode`] element.
    ///
    /// This function calculates the position of this [`OpticNode`] element in 3D space. This is based on the analysis of a single,
    /// central [`Ray`](crate::light::ray::Ray) representing the optical axis. The default implementation is to use the normal `analyze`
    /// function. For a [`NodeGroup`] however, this must be separately implemented in order to allow nesting. Within a graph,
    /// a node no axis reaches is passed over before this is called.
    ///
    /// # Errors
    /// This function will return an error if internal element-specific errors occur and the analysis cannot be performed.
    fn calc_node_positions(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        self.analyze(incoming_data, config)
    }
    /// Return the local frame from which the optical axis enters the given input port.
    ///
    /// During the positioning run the node's isometry is derived from the observed world frame `W`
    /// of the entering axis ray as `F = W ∘ L⁻¹`, where `L` is this frame. An ordinary node's axis
    /// enters along local +z through the local origin, so `L` is the identity and `F = W`. A node
    /// whose inputs do not share that convention overrides this — see
    /// [`BeamSplitter`](crate::nodes::BeamSplitter), whose second input arrives mirrored on the
    /// splitting plane, and [`BoredMirror`](crate::nodes::BoredMirror), whose rear input arrives
    /// along its bore.
    ///
    /// # Arguments
    ///
    /// * `port_name` - the input port the axis enters.
    ///
    /// # Returns
    ///
    /// The local entrance frame; the identity for every ordinary node.
    ///
    /// # Errors
    ///
    /// This function returns an error if the node cannot tell where the axis enters the port.
    fn axis_entrance_frame(&self, port_name: &str) -> OpmResult<Isometry> {
        let _ = port_name;
        Ok(Isometry::identity())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        core_optics::hit_map::fluence_estimator::FluenceEstimator,
        joule, millimeter,
        nodes::{
            Dummy, EnergyMeter, FluenceDetector, ParaxialSurface, SourcePort,
            round_collimated_ray_builder,
            test_helper::helper::{
                rays_at_meter, rays_recorded_by_meter,
                scenery_with_a_mirror_changed_behind_its_back,
                scenery_with_a_mirror_shrunk_behind_its_back,
            },
        },
        properties::Proptype,
        reporting::node_report::NodeReportResult,
        utils::test_helper::helper::{check_logs, reposition_in_vacuum},
    };
    use approx::assert_relative_eq;
    #[test]
    fn config_default() {
        let rt_conf = RayTraceConfig::default();
        assert_eq!(rt_conf.max_number_of_bounces(), 1000);
        assert_eq!(rt_conf.max_number_of_refractions(), 1000);
        assert_eq!(rt_conf.min_energy_per_ray(), picojoule!(1.0));
    }
    /// A property can change without its node rebuilding its surfaces: the backend writes it into
    /// the node's attributes directly. Placing the model rebuilds them first, so a ray trace
    /// traces a component as its properties state it now: an absolutely placed mirror shrunk to a
    /// radius of 5 mm no longer reflects the rays 8 mm off its axis, only the one on it.
    #[test]
    fn a_ray_trace_sees_a_component_as_its_properties_state_it_now() -> OpmResult<()> {
        let (mut scenery, source, meter) = scenery_with_a_mirror_shrunk_behind_its_back()?;
        let mut config = RayTraceConfig::default();
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(8.0), joule!(1.0), 1)?,
        );
        reposition_in_vacuum(&mut scenery)?;
        RayTracingAnalyzer::new(config).analyze(&mut scenery)?;
        assert_eq!(rays_recorded_by_meter(&scenery, meter)?, 1);
        Ok(())
    }
    /// A positioned scenery is traced where its nodes stand, without positioning it again: a
    /// distance changed after the positioning moves nothing, in a drawing or in an analysis, until
    /// the model is positioned anew.
    #[test]
    fn a_positioned_scenery_is_traced_where_its_nodes_stand() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let dummy = scenery.add_node(Dummy::default())?;
        scenery.connect_nodes(source, "output_1", dummy, "input_1", millimeter!(50.0))?;
        let mut config = RayTraceConfig::default();
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 0)?,
        );
        config.set_collect_ray_ends(true);
        AnalysisRayTrace::calc_node_positions(
            &mut scenery,
            LightResult::default(),
            &config.for_positioning(),
        )?;
        scenery.update_connection_distance(source, "output_1", millimeter!(80.0))?;
        let analyzer = RayTracingAnalyzer::new(config);
        // Where the light leaves the open output of the dummy, along the axis.
        let end_of = |scenery: &mut NodeGroup| -> OpmResult<f64> {
            let ends = scenery.take_ray_ends();
            let ray = ends.first().and_then(|end| end.iter().next());
            ray.map(|ray| ray.position().z.value)
                .ok_or_else(|| OpossumError::Other("no light left the dummy".into()))
        };

        analyzer.trace_positioned(&mut scenery, TracePurpose::Drawing)?;
        assert_relative_eq!(end_of(&mut scenery)?, 0.05, epsilon = 1e-9);

        analyzer.analyze(&mut scenery)?;
        assert_relative_eq!(end_of(&mut scenery)?, 0.05, epsilon = 1e-9);

        reposition_in_vacuum(&mut scenery)?;
        analyzer.analyze(&mut scenery)?;
        assert_relative_eq!(end_of(&mut scenery)?, 0.08, epsilon = 1e-9);
        Ok(())
    }
    /// A change that leaves a component without a valid shape stops the ray trace instead of
    /// tracing the shape the component had before: a mirror curved to R = 10 mm ends before its
    /// optic shape of 12.5 mm.
    #[test]
    fn a_ray_trace_stops_at_a_component_changed_into_no_valid_shape() -> OpmResult<()> {
        let (mut scenery, source, _) = scenery_with_a_mirror_changed_behind_its_back(
            "radius of curvature",
            Proptype::RadiusOfCurvature(millimeter!(-10.0)),
        )?;
        let mut config = RayTraceConfig::default();
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(8.0), joule!(1.0), 1)?,
        );
        let error = RayTracingAnalyzer::new(config)
            .analyze(&mut scenery)
            .expect_err("the mirror ends before its optic shape");
        assert!(error.to_string().contains("does not reach"), "{error}");
        Ok(())
    }
    /// The optical axis has to pass every component, so positioning stops a ray that misses one
    /// even when the ray trace itself lets missed rays run on.
    #[test]
    fn positioning_always_stops_a_ray_that_misses_a_surface() {
        let mut rt_conf = RayTraceConfig::default();
        rt_conf.set_missed_surface_strategy(MissedSurfaceStrategy::Ignore);
        let positioning = rt_conf.for_positioning();
        assert_eq!(
            positioning.missed_surface_strategy(),
            &MissedSurfaceStrategy::Stop
        );
        assert!(positioning.is_positioning_run());
    }
    /// The positioning run sends out the optical axis alone, so its sources span no ray tubes even
    /// where the run it precedes has them span some.
    #[test]
    fn the_positioning_run_spans_no_ray_tubes() {
        let mut rt_conf = RayTraceConfig::default();
        rt_conf.purpose = TracePurpose::Analysis { ray_tubes: true };
        assert!(rt_conf.needs_ray_tubes());
        assert!(!rt_conf.for_positioning().needs_ray_tubes());
    }
    /// Whether the rays a meter behind a source receives stand in ray tubes, with a fluence
    /// detector of the given estimator between them, or with none.
    fn rays_reach_the_meter_in_tubes(estimator: Option<FluenceEstimator>) -> OpmResult<bool> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let meter = scenery.add_node(EnergyMeter::default())?;
        if let Some(estimator) = estimator {
            let mut detector = FluenceDetector::default();
            detector
                .node_attr_mut()
                .set_property("fluence estimator", estimator.into())?;
            let detector = scenery.add_node(detector)?;
            scenery.connect_nodes(source, "output_1", detector, "input_1", millimeter!(10.0))?;
            scenery.connect_nodes(detector, "output_1", meter, "input_1", millimeter!(10.0))?;
        } else {
            scenery.connect_nodes(source, "output_1", meter, "input_1", millimeter!(10.0))?;
        }
        let mut config = RayTraceConfig::default();
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 1)?,
        );
        RayTracingAnalyzer::new(config).analyze(&mut scenery)?;
        let rays = rays_at_meter(&scenery, meter)?;
        let in_tubes = rays.iter().all(|ray| ray.source().is_some());
        assert_eq!(
            in_tubes,
            rays.iter().any(|ray| ray.source().is_some()),
            "all rays or none stand in tubes"
        );
        Ok(in_tubes)
    }
    /// The sources of a ray trace span ray tubes for a model with a fluence map that follows them,
    /// and only then.
    #[test]
    fn sources_span_ray_tubes_only_for_a_fluence_map_that_follows_them() -> OpmResult<()> {
        assert!(rays_reach_the_meter_in_tubes(Some(
            FluenceEstimator::Voronoi
        ))?);
        assert!(!rays_reach_the_meter_in_tubes(Some(
            FluenceEstimator::Binning
        ))?);
        assert!(!rays_reach_the_meter_in_tubes(None)?);
        Ok(())
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
            "RayTraceConfig { min_energy_per_ray: 1e-12 m^2 kg^1 s^-2, max_number_of_bounces: 1000, max_number_of_refractions: 1000, missed_surface_strategy: Stop, source_map: {}, ambient_material: Inline(Material { header: AssetHeader { schema_version: 1, id: 00000000-0000-0000-0000-000000000000, version: 0, name: \"vacuum\", manufacturer: None, description: None }, optical: OpticalProperties { refractive_index: Const(RefrIndexConst { refractive_index: Validated { value: 1.0, validator: AndValidator { v1: AllFinite, v2: StaticInRange { _marker: PhantomData<(f64, opossum_core::refractive_index::refr_index_const::RefIndBounds)> }, _marker: PhantomData<f64> } } }), absorption: None }, thermal: None, mechanical: None }), active_pump_scenario: ActiveScenario(None), positioning_run: false, collect_ray_ends: false, purpose: Drawing }"
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
            vec!["Performing ray tracing analysis of scenery 'test'."],
        );
        let mut scenery = NodeGroup::new("");
        let analyzer = RayTracingAnalyzer::default();
        testing_logger::setup();
        analyzer.analyze(&mut scenery)?;
        check_logs(
            log::Level::Info,
            vec!["Performing ray tracing analysis of scenery."],
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
        reposition_in_vacuum(&mut group)?;
        assert!(analyzer.analyze(&mut group).is_ok());
        let node_ref = group.graph().node(i_det)?;
        let NodeReportResult::Report(report) =
            node_ref.node_report("test_uuid", crate::analyzers::AnalyzerKind::RayTrace)?
        else {
            return Err(OpossumError::Analysis(
                "Should be NodeReportResult::Report".to_string(),
            ));
        };
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

        assert!(!config.map_source(uuid, source.clone().into()));
        assert_eq!(config.get_source(&uuid), Some(&source.clone().into()));

        // Let's use PointSrc for the second one to be sure it's different
        let source2 = RayDataSource::PointSrc(PointSrc::default());

        assert!(config.map_source(uuid, source2.clone().into()));
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
    /// Neither dummy is placed: no axis reaches either, and nothing either feeds is placed. Both
    /// are put back and, placed back to front, still have nothing to stand in front of.
    #[test]
    fn test_no_optical_axis_warning() -> OpmResult<()> {
        let mut scenery = NodeGroup::new("OpticScenery demo");
        let node1 = scenery.add_node(Dummy::new("dummy1"))?;
        let node2 = scenery.add_node(Dummy::new("dummy2"))?;
        scenery.connect_nodes(node1, "output_1", node2, "input_1", millimeter!(0.0))?;
        testing_logger::setup();
        reposition_in_vacuum(&mut scenery)?;
        check_logs(
            log::Level::Warn,
            vec![
                "'dummy1' (dummy) cannot be placed: no optical axis reaches it, and it stands in front of nothing it feeds. Skipping.",
                "'dummy2' (dummy) cannot be placed: no optical axis reaches it, and it stands in front of nothing it feeds. Skipping.",
            ],
        );
        Ok(())
    }
}
