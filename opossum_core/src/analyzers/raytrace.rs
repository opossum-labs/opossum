#![warn(missing_docs)]
//! Analyzer for sequential ray tracing
use std::{collections::HashMap, fmt::Display};

use super::{Analyzer, AnalyzerType};
use crate::{
    degree,
    error::{OpmResult, OpossumError},
    light_result::LightResult,
    lightdata::LightData,
    nodes::{NodeAttr, NodeGroup},
    optic_node::OpticNode,
    optic_ports::PortType,
    picojoule,
    prelude::RayDataBuilder,
    properties::Proptype,
    rays::Rays,
    refractive_index::RefractiveIndexType,
    reporting::analysis_report::AnalysisReport,
    utils::default_from_name::DefaultFromName,
};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AnalyzerRegistration;
use strum::EnumIter;

inventory::submit! {
    AnalyzerRegistration::new(
        || AnalyzerType::RayTrace(RayTraceConfig::default()),
        |at| if let AnalyzerType::RayTrace(config) = at { Some(Box::new(RayTracingAnalyzer::new(config.clone()))) } else { None }
    )
}
use uom::si::f64::{Angle, Energy, Length};

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
        AnalysisRayTrace::calc_node_positions(scenery, LightResult::default(), &self.config)?;
        scenery.reset_data();
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
    fn calc_node_positions(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        self.analyze(incoming_data, config)
    }

    /// Pass a bundle of rays through a surface
    /// # Arguments
    /// - `optic_surf_name`: the name of the surface
    /// - `refri_after_surf`: the refractive index after the surface
    /// - `rays_bundle`: a mutable reference to a vector of rays
    /// - `analyzer_type`: the analyzer type. needed to only evaluate fluences and store rays in caches for ghost focus analysis
    /// - `backward`: a flag that defines if the rays propagate in backward (true) or forward (false) direction
    /// # Errors
    /// This function errors if
    /// - no effctive isometry is defined for this node
    /// - the surface cannot be found
    /// - on error propagation
    fn pass_through_surface(
        &mut self,
        optic_surf_name: &str,
        refri_after_surf: &RefractiveIndexType,
        rays_bundle: &mut Vec<Rays>,
        analyzer_type: &AnalyzerType,
        backward: bool,
        refraction_intended: bool,
    ) -> OpmResult<()> {
        let uuid = self.node_attr().uuid();
        let iso = &self.effective_surface_iso(optic_surf_name)?;
        let Some(surf) = self.get_optic_surface_mut(optic_surf_name) else {
            return Err(OpossumError::Analysis(format!(
                "Cannot find surface: \"{optic_surf_name}\" of node: \"{}\"",
                self.node_attr().name()
            )));
        };
        let missed_surface_strategy = match analyzer_type {
            AnalyzerType::Energy(_) => &MissedSurfaceStrategy::Stop,
            AnalyzerType::RayTrace(ray_trace_config) => &ray_trace_config.missed_surface_strategy,
            AnalyzerType::GhostFocus(_) => &MissedSurfaceStrategy::Ignore,
        };
        for rays in &mut *rays_bundle {
            let mut reflected = rays.refract_on_surface(
                surf,
                Some(refri_after_surf),
                refraction_intended,
                missed_surface_strategy,
            )?;
            reflected.set_node_origin_uuid(uuid);
            if let AnalyzerType::GhostFocus(config) = analyzer_type {
                surf.evaluate_fluence_of_ray_bundle(rays, config.fluence_estimator())?;
                surf.add_to_rays_cache(reflected, backward);
            }

            rays.apodize(surf.aperture(), iso)?;
            if let AnalyzerType::RayTrace(config) = analyzer_type {
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
        }
        for rays in surf.get_rays_cache(backward) {
            rays_bundle.push(rays.clone());
        }
        Ok(())
    }

    /// Function to pass a bundle of rays through a detector surface.
    /// This function is used for the propagation through single surface detectors, such as a spot diagram
    /// # Attributes
    /// - `optic_surf_name`: the name of the [`OpticSurface`](crate::surface::optic_surface::OpticSurface)
    /// - `rays_bundle`: a mutable reference to a vector of [`Rays`],
    /// - `analyzer_type`: the analyzer type
    /// # Errors
    /// This function errors if the effective isometry is not defined
    fn pass_through_detector_surface(
        &mut self,
        optic_surf_name: &str,
        rays_bundle: &mut Vec<Rays>,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<()> {
        let optic_name = format!("'{}' ({})", self.name(), self.node_type());
        let mut apodized = false;
        let iso = self.effective_surface_iso(optic_surf_name)?;
        let Some(surf) = self.get_optic_surface_mut(optic_surf_name) else {
            return Err(OpossumError::Analysis("no surface found".into()));
        };
        let missed_surface_strategy = match analyzer_type {
            AnalyzerType::Energy(_) => &MissedSurfaceStrategy::Stop,
            AnalyzerType::RayTrace(ray_trace_config) => &ray_trace_config.missed_surface_strategy,
            AnalyzerType::GhostFocus(_) => &MissedSurfaceStrategy::Ignore,
        };
        for rays in &mut *rays_bundle {
            rays.refract_on_surface(surf, None, true, missed_surface_strategy)?;

            apodized |= rays.apodize(surf.aperture(), &iso)?;
            if apodized {
                warn!(
                    "Rays have been apodized at input aperture of {optic_name}. Results might not be accurate."
                );
            }
            if let AnalyzerType::GhostFocus(config) = analyzer_type {
                surf.evaluate_fluence_of_ray_bundle(rays, config.fluence_estimator())?;
            }
            if let AnalyzerType::RayTrace(c) = analyzer_type {
                rays.invalidate_by_threshold_energy(c.min_energy_per_ray)?;
            }
        }
        surf.prune_hit_map(&iso);
        self.set_apodization_warning(apodized);

        // merge all rays
        if let Some(ld) = self.get_light_data_mut() {
            if let LightData::GhostFocus(rays) = ld {
                for r in &*rays_bundle {
                    rays.push(r.clone());
                }
            }
            if let LightData::Geometric(rays) = ld {
                for r in &*rays_bundle {
                    rays.merge(r);
                }
            }
        } else {
            if let AnalyzerType::GhostFocus(_) = analyzer_type {
                self.set_light_data(LightData::GhostFocus(rays_bundle.clone()));
            }
            if let AnalyzerType::RayTrace(_) = analyzer_type {
                self.set_light_data(LightData::Geometric(rays_bundle[0].clone()));
            }
        }
        Ok(())
    }

    /// Effectively the analyze function of detector nodes with a single surface for a ray-tracing analysis
    /// Helper function to reduce code-doubling
    /// # Attributes
    /// - `incoming_data`: the incoming data for this anaylsis in form of a `LightResult`
    /// - `config`: the [`RayTraceConfig`] of this analysis
    /// # Errors
    /// This function errors if `pass_through_detector_surface` fails
    fn analyze_single_surface_node(
        &mut self,
        mut incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        // Find input & output port name
        // We could also use self.ports().name() but this is much slower (memory allocation) ...
        let (in_port_name, out_port_name) = {
            // Wir holen uns kurz den Lese-Zugriff
            let raw_ports = self.node_attr().ports();
            let is_inverted = self.inverted();
            let lookup_input_type = if is_inverted {
                PortType::Output
            } else {
                PortType::Input
            };
            let lookup_output_type = if is_inverted {
                PortType::Input
            } else {
                PortType::Output
            };
            let in_map = raw_ports.ports(&lookup_input_type);
            let out_map = raw_ports.ports(&lookup_output_type);

            let in_key = in_map
                .keys()
                .next()
                .ok_or_else(|| OpossumError::Analysis("Node hat keinen Input-Port".into()))?;
            let out_key = out_map
                .keys()
                .next()
                .ok_or_else(|| OpossumError::Analysis("Node hat keinen Output-Port".into()))?;
            (in_key.clone(), out_key.clone())
        };

        let Some(data) = incoming_data.remove(&in_port_name) else {
            return Ok(LightResult::default());
        };
        if let LightData::Geometric(rays) = data {
            let mut rays_bundle = vec![rays];
            self.pass_through_detector_surface(
                &in_port_name,
                &mut rays_bundle,
                &AnalyzerType::RayTrace(config.clone()),
            )?;
            Ok(LightResult::from([(
                out_port_name,
                self.get_light_data_mut().unwrap().clone(),
            )]))
        } else {
            Ok(LightResult::from([(out_port_name, data)]))
        }
    }

    ///returns a mutable reference to the light data.
    fn get_light_data_mut(&mut self) -> Option<&mut LightData> {
        None
    }

    ///sets the light data field of this detector
    fn set_light_data(&mut self, _ld: LightData) {}

    ///returns the necessary node attributes for ray tracing
    /// # Errors
    /// This function errors if the node attributes: Isometry, Refractive Index or Center Thickness cannot be read,
    fn get_node_attributes_ray_trace(
        &self,
        node_attr: &NodeAttr,
    ) -> OpmResult<(RefractiveIndexType, Length, Angle)> {
        let Ok(Proptype::RefractiveIndex(index_model)) = node_attr.get_property("refractive index")
        else {
            return Err(OpossumError::Analysis(
                "cannot read refractive index".into(),
            ));
        };
        let Ok(Proptype::Length(center_thickness)) = node_attr.get_property("center thickness")
        else {
            return Err(OpossumError::Analysis(
                "cannot read center thickness".into(),
            ));
        };

        let angle = if let Ok(Proptype::Angle(wedge)) = node_attr.get_property("wedge") {
            *wedge
        } else {
            degree!(0.)
        };

        Ok((index_model.clone(), *center_thickness, angle))
    }
}

/// Strategy to use if a [`Ray`](crate::ray::Ray) misses a surface
#[derive(Default, PartialEq, Eq, Debug, Clone, Copy, Serialize, Deserialize, EnumIter)]
pub enum MissedSurfaceStrategy {
    /// The [`Ray`](crate::ray::Ray) it is set as invalid and does no longer propagate.
    #[default]
    Stop,
    /// The [`Ray`](crate::ray::Ray) is not altered in any way, thus skipping the surface and propagating
    /// further through the system.
    Ignore,
}
impl DefaultFromName for MissedSurfaceStrategy {}
impl Display for MissedSurfaceStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stop => write!(f, "Stop"),
            Self::Ignore => write!(f, "Ignore"),
        }
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
/// Configuration data for a rays tracing analysis.
///
/// The config contains the following info
// ///  - ray tracing mode (see [`RayTracingMode`])
///   - minimum energy / ray
///   - maximum number of bounces (reflections) / ray
///   - maximum number of refractions / ray
pub struct RayTraceConfig {
    //mode: RayTracingMode,
    min_energy_per_ray: Energy,
    max_number_of_bounces: usize,
    max_number_of_refractions: usize,
    missed_surface_strategy: MissedSurfaceStrategy,
    source_map: HashMap<Uuid, RayDataBuilder>,
}
impl Default for RayTraceConfig {
    /// Create a default config for a ray tracing analysis with the following parameters:
    ///   - mininum energy / ray: `1 pJ`
    ///   - maximum number of bounces / ray: `1000`
    ///   - maximum number of refractions / ray: `1000`
    ///   - missed surface strategy: ray is stopped
    fn default() -> Self {
        Self {
            min_energy_per_ray: picojoule!(1.0),
            max_number_of_bounces: 1000,
            max_number_of_refractions: 1000,
            missed_surface_strategy: MissedSurfaceStrategy::default(),
            source_map: HashMap::new(),
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
    /// If a builder was already mapped to the given UUID, it is replaced and returned.
    pub fn map_source(&mut self, uuid: Uuid, builder: RayDataBuilder) -> Option<RayDataBuilder> {
        self.source_map.insert(uuid, builder)
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
    /// Removes all source mappings whose UUIDs no longer exist in the given model.
    pub fn prune_source_map(&mut self, model: &NodeGroup) {
        self.source_map.retain(|uuid, _builder| model.exists(*uuid));
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        joule, millimeter,
        nodes::{ParaxialSurface, round_collimated_ray_source},
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
            "RayTraceConfig { min_energy_per_ray: 1e-12 m^2 kg^1 s^-2, max_number_of_bounces: 1000, max_number_of_refractions: 1000, missed_surface_strategy: Stop, source_map: {} }"
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
    fn analyze_info() {
        let mut scenery = NodeGroup::new("test");
        let analyzer = RayTracingAnalyzer::default();
        testing_logger::setup();
        analyzer.analyze(&mut scenery).unwrap();
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
        analyzer.analyze(&mut scenery).unwrap();
        check_logs(
            log::Level::Info,
            vec![
                "Calculate node positions of scenery.",
                "Performing ray tracing analysis of scenery.",
            ],
        );
    }
    #[test]
    fn report() {
        let analyzer = RayTracingAnalyzer::default();
        let scenery = NodeGroup::new("");
        analyzer.report(&scenery).unwrap();
    }
    #[test]
    #[ignore]
    fn integration_test() {
        // simulate simple system for integration test
        let mut group = NodeGroup::default();
        let i_src = group
            .add_node(round_collimated_ray_source(millimeter!(10.0), joule!(1.0), 3).unwrap())
            .unwrap();
        let i_l1 = group
            .add_node(ParaxialSurface::new("f=100", millimeter!(100.0)).unwrap())
            .unwrap();
        group
            .connect_nodes(i_src, "output_1", i_l1, "input_1", millimeter!(50.0))
            .unwrap();
        let analyzer = RayTracingAnalyzer::default();
        analyzer.analyze(&mut group).unwrap();
    }

    #[test]
    fn test_map_and_get_source() {
        use crate::lightdata::ray_data_builder::{CollimatedSrc, PointSrc, RayDataBuilder};
        use uuid::Uuid;
        let mut config = RayTraceConfig::default();
        let uuid = Uuid::new_v4();
        // Use CollimatedSrc which implements Default
        let builder = RayDataBuilder::Collimated(CollimatedSrc::default());

        assert!(config.map_source(uuid, builder.clone()).is_none());
        assert_eq!(config.get_source(&uuid), Some(&builder));

        let mut builder2 = RayDataBuilder::Collimated(CollimatedSrc::default());
        if let RayDataBuilder::Collimated(ref mut _c) = builder2 {
            // Modify it slightly to be different, though default equality check might suffice if we just use a different instance
            // For safety let's just use the same type but maybe a different property if I could easy set it.
            // But valid builders are complex to construct manually due to validators.
            // Let's just trust that a new default is equal to another new default, so we need to validly change something.
            // or just use a PointSrc for the second one.
        }
        // Let's use PointSrc for the second one to be sure it's different
        let builder2 = RayDataBuilder::PointSrc(PointSrc::default());

        assert_eq!(config.map_source(uuid, builder2.clone()), Some(builder));
        assert_eq!(config.get_source(&uuid), Some(&builder2));
    }

    #[test]
    fn test_remove_source() {
        use crate::lightdata::ray_data_builder::{CollimatedSrc, RayDataBuilder};
        use uuid::Uuid;
        let mut config = RayTraceConfig::default();
        let uuid = Uuid::new_v4();
        let builder = RayDataBuilder::Collimated(CollimatedSrc::default());

        config.map_source(uuid, builder.clone());
        assert_eq!(config.remove_source(&uuid), Some(builder));
        assert!(config.get_source(&uuid).is_none());
        assert!(config.remove_source(&uuid).is_none());
    }

    #[test]
    fn test_prune_source_map() {
        use crate::{
            lightdata::ray_data_builder::{CollimatedSrc, RayDataBuilder},
            nodes::Source,
            prelude::LightDataBuilder, // Correct import
        };
        use uuid::Uuid;
        let mut config = RayTraceConfig::default();
        let uuid2 = Uuid::new_v4();
        let builder = RayDataBuilder::Collimated(CollimatedSrc::default());

        let mut scene = NodeGroup::default();
        let src = Source::new("source", LightDataBuilder::Geometric(builder.clone()));
        let node_id = scene.add_node(src).unwrap();

        config.map_source(node_id, builder.clone());
        config.map_source(uuid2, builder.clone());

        config.prune_source_map(&scene);

        assert!(config.get_source(&node_id).is_some());
        assert!(config.get_source(&uuid2).is_none());
    }
}
