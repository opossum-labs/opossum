#![warn(missing_docs)]
//! fluence measurement node
pub mod fluence_data;

use super::node_attr::NodeAttr;
use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
        Analyzable, GhostFocusConfig, RayTraceConfig,
    },
    dottable::Dottable,
    error::OpmResult,
    light_result::{LightRays, LightResult},
    lightdata::LightData,
    optic_node::{Alignable, OpticNode, LIDT},
    optic_ports::PortType,
    properties::{Properties, Proptype},
    rays::Rays,
    reporting::node_report::NodeReport,
    surface::hit_map::FluenceEstimator,
};
use log::warn;

/// alias for uom `RadiantExposure`, as this name is rather uncommon to use for laser scientists
pub type Fluence = uom::si::f64::RadiantExposure;

/// A fluence monitor
///
/// It simply calculates the fluence (spatial energy distribution) of an incoming [`Ray`](crate::ray::Ray) bundle. The used algorithm
/// for calculating a fluence map is specified with the property `fluence estimator`. By default, the Voronoi estimator is
/// used ([`FluenceEstimator::Voronoi`]). See [`FluenceEstimator`] for further options.
///
/// ## Optical Ports
///   - Inputs
///     - `in1`
///   - Outputs
///     - `out1`
///
/// ## Properties
///   - `name`
///   - `fluence estimator`
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere within the optical setup.
#[derive(Clone, Debug)]
pub struct FluenceDetector {
    node_attr: NodeAttr,
    apodization_warning: bool,
    light_data: Option<LightData>,
}
impl Default for FluenceDetector {
    /// creates a fluence detector.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("fluence detector");
        node_attr
            .create_property(
                "fluence estimator",
                "fluence estimator strategy",
                None,
                FluenceEstimator::Voronoi.into(),
            )
            .unwrap();
        let mut fld = Self {
            node_attr,
            apodization_warning: false,
            light_data: None,
        };
        fld.update_surfaces().unwrap();
        fld
    }
}
impl FluenceDetector {
    /// Creates a new [`FluenceDetector`].
    /// # Attributes
    /// * `name`: name of the fluence detector
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut fld = Self::default();
        fld.node_attr.set_name(name);
        fld
    }
}

impl OpticNode for FluenceDetector {
    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
    fn node_report(&self, uuid: &str) -> Option<NodeReport> {
        let mut props = Properties::default();
        let hit_maps = self.hit_maps();
        let Some(hit_map) = hit_maps.get("input_1") else {
            warn!("could not get surface hitmap using default");
            return None;
        };
        let Ok(Proptype::FluenceEstimator(estimator)) =
            self.node_attr.get_property("fluence estimator")
        else {
            return None;
        };
        if let Ok(fluence_data_kde) = hit_map
            .get_merged_rays_hit_map()
            .calc_fluence_map((50, 50), estimator)
        {
            props
                .create(
                    &format!("Fluence ({estimator})"),
                    "2D spatial energy distribution",
                    None,
                    fluence_data_kde.clone().into(),
                )
                .unwrap();
            props
                .create(
                    &format!("Peak Fluence ({estimator})"),
                    "Peak fluence of the distribution",
                    None,
                    Proptype::Fluence(fluence_data_kde.peak()),
                )
                .unwrap();
            props
                .create(
                    &format!("Total energy ({estimator})"),
                    "Total energy of the distribution",
                    None,
                    Proptype::Energy(fluence_data_kde.total_energy()),
                )
                .unwrap();
            if self.apodization_warning {
                props
                    .create(
                        "Warning",
                        "warning during analysis",
                        None,
                        "Rays have been apodized at input aperture. Results might not be accurate."
                            .into(),
                    )
                    .unwrap();
            }
        }
        Some(NodeReport::new(
            &self.node_type(),
            &self.name(),
            uuid,
            props,
        ))
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn reset_data(&mut self) {
        // self.light_data = None;
        self.reset_optic_surfaces();
    }
}

impl Alignable for FluenceDetector {}
impl Dottable for FluenceDetector {
    fn node_color(&self) -> &str {
        "hotpink"
    }
}
impl LIDT for FluenceDetector {}
impl Analyzable for FluenceDetector {}
impl AnalysisGhostFocus for FluenceDetector {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        AnalysisGhostFocus::analyze_single_surface_node(self, incoming_data, config)
    }
}
impl AnalysisEnergy for FluenceDetector {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(data) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        Ok(LightResult::from([(out_port.into(), data.clone())]))
    }
}
impl AnalysisRayTrace for FluenceDetector {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        AnalysisRayTrace::analyze_single_surface_node(self, incoming_data, config)
    }
    fn get_light_data_mut(&mut self) -> Option<&mut LightData> {
        self.light_data.as_mut()
    }
    fn set_light_data(&mut self, ld: LightData) {
        self.light_data = Some(ld);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::lightdata::LightData;
    use crate::optic_ports::PortType;
    use crate::{
        lightdata::DataEnergy, nodes::test_helper::test_helper::*,
        spectrum_helper::create_he_ne_spec,
    };
    #[test]
    fn default() {
        let mut node = FluenceDetector::default();
        assert_eq!(node.name(), "fluence detector");
        assert_eq!(node.node_type(), "fluence detector");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "hotpink");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let meter = FluenceDetector::new("test");
        assert_eq!(meter.name(), "test");
    }
    #[test]
    fn ports() {
        let meter = FluenceDetector::default();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = FluenceDetector::default();
        meter.set_inverted(true).unwrap();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["input_1"]);
    }
    #[test]
    fn inverted() {
        test_inverted::<FluenceDetector>()
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<FluenceDetector>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = FluenceDetector::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("wrong".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_ok() {
        let mut node = FluenceDetector::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_apodization_warning() {
        test_analyze_apodization_warning::<FluenceDetector>()
    }
    #[test]
    fn analyze_inverse() {
        let mut node = FluenceDetector::default();
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("output_1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
}
