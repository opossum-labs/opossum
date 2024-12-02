//! Analyzer for sequential ray tracing
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
    properties::Proptype,
    rays::Rays,
    refractive_index::RefractiveIndexType,
    reporting::analysis_report::AnalysisReport,
};
use log::{info, warn};
use serde::{Deserialize, Serialize};
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
        AnalysisRayTrace::calc_node_position(scenery, LightResult::default(), &self.config)?;
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
    fn calc_node_position(
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
        let uuid = *self.node_attr().uuid();
        let iso = &self.effective_surface_iso(optic_surf_name)?;
        let Some(surf) = self.get_optic_surface_mut(optic_surf_name) else {
            return Err(OpossumError::Analysis(format!(
                "Cannot find surface: \"{optic_surf_name}\" of node: \"{}\"",
                self.node_attr().name()
            )));
        };
        for rays in &mut *rays_bundle {
            let mut reflected =
                rays.refract_on_surface(surf, Some(refri_after_surf), refraction_intended)?;
            reflected.set_node_origin_uuid(uuid);
            if let AnalyzerType::GhostFocus(_) = analyzer_type {
                surf.evaluate_fluence_of_ray_bundle(rays)?;
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
        for rays in &mut *rays_bundle {
            rays.refract_on_surface(surf, None, true)?;

            apodized |= rays.apodize(surf.aperture(), &iso)?;
            if apodized {
                warn!("Rays have been apodized at input aperture of {}. Results might not be accurate.", optic_name);
            }
            if let AnalyzerType::GhostFocus(_) = analyzer_type {
                surf.evaluate_fluence_of_ray_bundle(rays)?;
            }
            if let AnalyzerType::RayTrace(c) = analyzer_type {
                rays.invalidate_by_threshold_energy(c.min_energy_per_ray)?;
            }
        }

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
    /// - `incoming_data`: the incoming data for this anaylsis in form of a [`LightResult`]
    /// - `config`: the [`RayTraceConfig`] of this analysis
    /// # Errors
    /// This function errors if `pass_through_detector_surface` fails
    fn analyze_single_surface_node(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];

        let Some(data) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        if let LightData::Geometric(rays) = data {
            self.pass_through_detector_surface(
                in_port,
                &mut vec![rays.clone()],
                &AnalyzerType::RayTrace(config.clone()),
            )?;
            Ok(LightResult::from([(
                out_port.into(),
                self.get_light_data_mut().unwrap().clone(),
            )]))
        } else {
            Ok(LightResult::from([(out_port.into(), data.clone())]))
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

        Ok((index_model.value.clone(), *center_thickness, angle))
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
// ///  - ray tracing mode (see [`RayTracingMode`])
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
    // ///   - ray tracing mode: [`RayTracingMode::Sequential`]
    ///   - mininum energy / ray: `1 pJ`
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
            "RayTraceConfig { min_energy_per_ray: 1e-12 m^2 kg^1 s^-2, max_number_of_bounces: 1000, max_number_of_refractions: 1000 }"
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
    fn integration_test() {
        // simulate simple system for integration test
        let mut group = NodeGroup::default();
        let i_src = group
            .add_node(&round_collimated_ray_source(millimeter!(10.0), joule!(1.0), 3).unwrap())
            .unwrap();
        let i_l1 = group
            .add_node(&ParaxialSurface::new("f=100", millimeter!(100.0)).unwrap())
            .unwrap();
        group
            .connect_nodes(i_src, "output_1", i_l1, "input_1", millimeter!(50.0))
            .unwrap();
        let analyzer = RayTracingAnalyzer::default();
        analyzer.analyze(&mut group).unwrap();
    }
}
