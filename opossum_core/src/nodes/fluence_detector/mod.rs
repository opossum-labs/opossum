#![warn(missing_docs)]
//! fluence measurement node
pub mod fluence_data;

use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
    },
    core_optics::{NodeAttr, OpticNode, hit_map::fluence_estimator::FluenceEstimator},
    error::OpmResult,
    light::LightData,
    nodes::NodeRegistration,
    properties::{Properties, Proptype},
    reporting::node_report::NodeReport,
};
use log::warn;
use opm_macros_lib::OpmNode;

/// alias for uom `RadiantExposure`, as this name is rather uncommon to use for laser scientists
pub type Fluence = uom::si::f64::RadiantExposure;

inventory::submit! {
    NodeRegistration::new::<FluenceDetector>("fluence detector", "fluence detector")
}

/// A fluence monitor
///
/// It simply calculates the fluence (spatial energy distribution) of an incoming [`Ray`](crate::light::Ray) bundle. The used algorithm
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
#[derive(OpmNode, Clone, Debug)]
#[opm_node("hotpink")]
pub struct FluenceDetector {
    node_attr: NodeAttr,
    apodization_warning: bool,
    light_data: Option<LightData>,
}
unsafe impl Send for FluenceDetector {}
impl Default for FluenceDetector {
    /// creates a fluence detector.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("fluence detector");
        node_attr
            .create_property(
                "fluence estimator",
                "fluence estimator strategy",
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

        let mut consolidated_hit_map = hit_map.clone();
        consolidated_hit_map.consolidate();

        let Ok(Proptype::FluenceEstimator(estimator)) =
            self.node_attr.get_property("fluence estimator")
        else {
            return None;
        };

        let fl_data = consolidated_hit_map.calc_fluence_map((95, 83), estimator);

        if let Ok(ref fluence_data) = fl_data {
            props
                .create(
                    &format!("Fluence ({})", fluence_data.estimator()),
                    "2D spatial energy distribution",
                    fluence_data.clone().into(),
                )
                .unwrap();
            props
                .create(
                    &format!("Peak Fluence ({})", fluence_data.estimator()),
                    "Peak fluence of the distribution",
                    Proptype::Fluence(fluence_data.peak()),
                )
                .unwrap();
            props
                .create(
                    &format!("Total energy ({})", fluence_data.estimator()),
                    "Total energy of the distribution",
                    Proptype::Energy(fluence_data.total_energy()),
                )
                .unwrap();
            if self.apodization_warning {
                props
                    .create(
                        "Warning",
                        "warning during analysis",
                        "Rays have been apodized at input aperture. Results might not be accurate."
                            .into(),
                    )
                    .unwrap();
            }
        } else {
            warn!(
                "Error while trying to calculate the fluence map with the defined estimator. Plot is omitted."
            );
            props
                .create(
                    "Warning",
                    "warning during analysis",
                    "Could not calculate the fluence map with the defined estimator. Please try another one"
                        .into(),
                )
                .unwrap();
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
        self.light_data = None;
        self.reset_optic_surfaces();
    }
    fn set_light_data(&mut self, ld: LightData) {
        self.light_data = Some(ld);
    }
}
impl AnalysisGhostFocus for FluenceDetector {}
impl AnalysisEnergy for FluenceDetector {}
impl AnalysisRayTrace for FluenceDetector {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::energy::EnergyConfig,
        core_optics::PortType,
        light::{LightData, LightResult, spectrum_helper::create_he_ne_spec},
        nodes::test_helper::test_helper::*,
    };
    #[test]
    fn default() {
        let mut node = FluenceDetector::default();
        assert_eq!(node.name(), "fluence detector");
        assert_eq!(node.node_type(), "fluence detector");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "hotpink");
        assert!(node.as_group_mut().is_err());
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
    fn inverted() -> OpmResult<()> {
        test_inverted::<FluenceDetector>()
    }
    #[test]
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<FluenceDetector>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = FluenceDetector::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("wrong".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default()).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_ok() {
        let mut node = FluenceDetector::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default()).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_apodization_warning() -> OpmResult<()> {
        test_analyze_apodization_warning::<FluenceDetector>()
    }
    #[test]
    fn analyze_inverse() {
        let mut node = FluenceDetector::default();
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("output_1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default()).unwrap();
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
}
