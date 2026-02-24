#![warn(missing_docs)]
use super::node_attr::NodeAttr;
use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
    },
    error::OpmResult,
    joule,
    lightdata::LightData,
    nodes::NodeRegistration,
    optic_node::OpticNode,
    properties::{Properties, Proptype},
    reporting::node_report::NodeReport,
};
use log::warn;
use opm_macros_lib::OpmNode;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};

#[non_exhaustive]
#[derive(Debug, Default, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
/// Type of the [`EnergyMeter`]. This is currently not used.
pub enum Metertype {
    /// an ideal energy meter
    #[default]
    IdealEnergyMeter,
    /// an ideal power meter (currently not used)
    IdealPowerMeter,
}
impl Display for Metertype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IdealEnergyMeter => write!(f, "ideal energy meter"),
            Self::IdealPowerMeter => write!(f, "ideal power meter"),
        }
    }
}
impl From<Metertype> for Proptype {
    fn from(value: Metertype) -> Self {
        Self::Metertype(value)
    }
}

inventory::submit! {
    NodeRegistration::new::<EnergyMeter>("energy meter", "ideal energy meter")
}
/// An (ideal) energy / power meter.
///
/// It normally measures the total energy of the incoming light regardless of the wavelength, position, angle, polarization etc...
///
/// ## Optical Ports
///   - Inputs
///     - `in1`
///   - Outputs
///     - `out1`
///
/// ## Propertied
///   - `name`
///   - `inverted`
///   - `meter type`
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere in between arbitrary optic nodes.
#[derive(OpmNode, Clone)]
#[opm_node("whitesmoke")]
pub struct EnergyMeter {
    light_data: Option<LightData>,
    node_attr: NodeAttr,
    apodization_warning: bool,
}
unsafe impl Send for EnergyMeter {}

impl Default for EnergyMeter {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("energy meter");
        node_attr
            .create_property(
                "meter type",
                "model type of the meter",
                Metertype::default().into(),
            )
            .unwrap();
        let mut em = Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
        };
        em.update_surfaces().unwrap();
        em
    }
}
impl EnergyMeter {
    /// Creates a new [`EnergyMeter`] of the given [`Metertype`].
    /// # Attributes
    /// * `name`:           name of the [`EnergyMeter`]
    /// * `meter_type`:     specific [`Metertype`] of the [`EnergyMeter`]
    ///
    /// # Panics
    /// This function panics if the [`Properties`] `name` or `meter type` can not be set.
    #[must_use]
    pub fn new(name: &str, meter_type: Metertype) -> Self {
        let mut energy_meter = Self::default();
        energy_meter.node_attr.set_name(name);
        energy_meter
            .node_attr
            .set_property("meter type", meter_type.into())
            .unwrap();
        energy_meter
    }
    /// Returns the meter type of this [`EnergyMeter`].
    /// # Panics
    /// This function panics if
    /// - the property "meter type" does not exist.
    /// - the data format is wrong.
    #[must_use]
    pub fn meter_type(&self) -> Metertype {
        if let Ok(Proptype::Metertype(meter_type)) = self.node_attr.get_property("meter type") {
            *meter_type
        } else {
            panic!("wrong data format")
        }
    }
    /// Sets the meter type of this [`EnergyMeter`].
    /// # Panics
    /// This function panics if the property "meter type" can not be set.
    pub fn set_meter_type(&mut self, meter_type: Metertype) {
        self.node_attr
            .set_property("meter type", meter_type.into())
            .unwrap();
    }
}
impl OpticNode for EnergyMeter {
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
    fn node_report(&self, uuid: &str) -> Option<NodeReport> {
        let energy = self
            .light_data
            .as_ref()
            .and_then(|light_data| match light_data {
                LightData::Energy(s) => Some(joule!(s.total_energy())),
                LightData::Geometric(r) => Some(r.total_energy()),
                LightData::Fourier => None,
                LightData::GhostFocus(r) => {
                    let mut energy = joule!(0.);
                    for rays in r {
                        energy += rays.total_energy();
                    }
                    Some(energy)
                }
            });
        let mut props = Properties::default();
        if let Some(e) = energy {
            props.create("Energy", "Output energy", e.into()).unwrap();
        } else {
            props
                .create("Energy", "Output energy", "no data".into())
                .unwrap();
        }
        props
            .create(
                "Model",
                "type of meter",
                self.node_attr.get_property("meter type").unwrap().clone(),
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
    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }
    fn get_light_data_mut(&mut self) -> Option<&mut LightData> {
        self.light_data.as_mut()
    }
    fn set_light_data(&mut self, new_data: LightData) {
        self.light_data = Some(new_data);
    }
}

impl Debug for EnergyMeter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.light_data {
            Some(data) => write!(f, "{} (Type: {:?})", data, self.meter_type()),
            None => write!(f, "no data"),
        }
    }
}
impl AnalysisGhostFocus for EnergyMeter {}
impl AnalysisEnergy for EnergyMeter {}
impl AnalysisRayTrace for EnergyMeter {}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::energy::EnergyConfig, light_result::LightResult,
        nodes::test_helper::test_helper::*, optic_ports::PortType,
        spectrum_helper::create_he_ne_spec,
    };
    #[test]
    fn default() {
        let mut node = EnergyMeter::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.meter_type(), Metertype::IdealEnergyMeter);
        assert_eq!(node.name(), "energy meter");
        assert_eq!(node.node_type(), "energy meter");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "whitesmoke");
        assert!(node.as_group_mut().is_err());
    }
    #[test]
    fn new() {
        let meter = EnergyMeter::new("test", Metertype::IdealPowerMeter);
        assert!(meter.light_data.is_none());
        assert_eq!(meter.meter_type(), Metertype::IdealPowerMeter);
        assert_eq!(meter.name(), "test");
    }
    #[test]
    fn inverted() {
        test_inverted::<EnergyMeter>()
    }
    #[test]
    fn set_meter_type() {
        let mut meter = EnergyMeter::default();
        meter.set_meter_type(Metertype::IdealPowerMeter);
        assert_eq!(meter.meter_type(), Metertype::IdealPowerMeter);
    }
    #[test]
    fn ports() {
        let meter = EnergyMeter::default();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = EnergyMeter::default();
        meter.set_inverted(true).unwrap();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["input_1"]);
    }
    #[test]
    fn set_aperture() {
        test_set_aperture::<EnergyMeter>("input_1", "output_1");
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<EnergyMeter>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = EnergyMeter::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default()).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_ok() {
        let mut meter = EnergyMeter::default();
        let mut input = LightResult::default();
        let input_data = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("input_1".into(), input_data.clone());
        let result = AnalysisEnergy::analyze(&mut meter, input, &EnergyConfig::default()).unwrap();
        assert!(result.contains_key("output_1"));
        assert_eq!(result.get("output_1").unwrap(), &input_data);
    }
    #[test]
    fn analyze_apodization_warning() {
        test_analyze_apodization_warning::<EnergyMeter>()
    }
    #[test]
    fn analyze_inverted() {
        let mut meter = EnergyMeter::default();
        let mut input = LightResult::default();
        meter.set_inverted(true).unwrap();
        let input_data = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("output_1".into(), input_data.clone());
        let result = AnalysisEnergy::analyze(&mut meter, input, &EnergyConfig::default()).unwrap();
        assert!(result.contains_key("input_1"));
        assert_eq!(result.get("input_1").unwrap(), &input_data);
    }
    #[test]
    fn debug() {
        let mut meter = EnergyMeter::default();
        assert_eq!(format!("{meter:?}"), "no data");
        let mut input = LightResult::default();
        let input_data = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("input_1".into(), input_data.clone());
        AnalysisEnergy::analyze(&mut meter, input, &EnergyConfig::default()).unwrap();
        assert_eq!(format!("{meter:?}"), "Energy: 1 J (Type: IdealEnergyMeter)");
    }
    #[test]
    fn report() {
        let mut meter = EnergyMeter::default();
        let report = meter.node_report("123").unwrap();
        assert_eq!(report.name(), "energy meter");
        assert_eq!(report.node_type(), "energy meter");
        assert!(report.properties().contains("Energy"));
        assert!(report.properties().contains("Model"));
        if let Ok(Proptype::String(s)) = report.properties().get("Energy") {
            assert_eq!(s, "no data");
        } else {
            panic!("could not read Energy property");
        }
        if let Ok(Proptype::Metertype(t)) = report.properties().get("Model") {
            assert_eq!(t, &Metertype::IdealEnergyMeter);
        } else {
            panic!("could not read Model property");
        }
        let mut input = LightResult::default();
        let input_data = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("input_1".into(), input_data.clone());
        AnalysisEnergy::analyze(&mut meter, input, &EnergyConfig::default()).unwrap();
        let report = meter.node_report("123").unwrap();
        if let Ok(Proptype::Energy(e)) = report.properties().get("Energy") {
            assert_eq!(e, &joule!(1.0));
        } else {
            panic!("could not read Energy property");
        }
    }
}
