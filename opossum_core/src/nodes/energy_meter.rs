#![warn(missing_docs)]
use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
    },
    core_optics::{NodeAttr, NodeAttrExt, OpticNode, OpticNodeExt},
    error::OpmResult,
    joule,
    light::LightData,
    nodes::NodeRegistration,
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
    /// # Errors
    ///
    /// This function returns an error if the [`Properties`] `name` or `meter type` can not be set.
    pub fn new(name: &str, meter_type: Metertype) -> OpmResult<Self> {
        let mut energy_meter = Self::default();
        energy_meter.node_attr.set_name(name);
        energy_meter
            .node_attr
            .set_property("meter type", meter_type.into())?;
        Ok(energy_meter)
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
    ///
    /// # Errors
    ///
    /// This function returns an error if internally the property "meter type" can not be set.
    pub fn set_meter_type(&mut self, meter_type: Metertype) -> OpmResult<()> {
        self.node_attr
            .set_property("meter type", meter_type.into())?;
        Ok(())
    }
}
impl OpticNode for EnergyMeter {
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
    fn node_report(&self, uuid: &str) -> OpmResult<Option<NodeReport>> {
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
            props.create("Energy", "Output energy", e.into())?;
        } else {
            props.create("Energy", "Output energy", "no data".into())?;
        }
        props.create(
            "Model",
            "type of meter",
            self.node_attr.get_property("meter type")?.clone(),
        )?;
        if self.apodization_warning {
            props.create(
                "Warning",
                "warning during analysis",
                "Rays have been apodized at input aperture. Results might not be accurate.".into(),
            )?;
        }
        Ok(Some(NodeReport::new(
            self.node_type(),
            self.name(),
            uuid,
            props,
        )))
    }
    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }
    fn set_light_data(&mut self, new_data: Option<LightData>) {
        self.light_data = new_data;
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
        analyzers::energy::EnergyConfig,
        core_optics::PortType,
        light::{LightResult, spectrum_helper::create_he_ne_spec},
        nodes::test_helper::test_helper::*,
    };
    #[test]
    fn default() {
        let node = EnergyMeter::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.meter_type(), Metertype::IdealEnergyMeter);
        assert_eq!(node.name(), "energy meter");
        assert_eq!(node.node_type(), "energy meter");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "whitesmoke");
    }
    #[test]
    fn new() -> OpmResult<()> {
        let meter = EnergyMeter::new("test", Metertype::IdealPowerMeter)?;
        assert!(meter.light_data.is_none());
        assert_eq!(meter.meter_type(), Metertype::IdealPowerMeter);
        assert_eq!(meter.name(), "test");
        Ok(())
    }
    #[test]
    fn inverted() -> OpmResult<()> {
        test_inverted::<EnergyMeter>()
    }
    #[test]
    fn set_meter_type() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        meter.set_meter_type(Metertype::IdealPowerMeter)?;
        assert_eq!(meter.meter_type(), Metertype::IdealPowerMeter);
        Ok(())
    }
    #[test]
    fn ports() {
        let meter = EnergyMeter::default();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        meter.set_inverted(true)?;
        assert_eq!(meter.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["input_1"]);
        Ok(())
    }
    #[test]
    fn set_aperture() {
        test_set_aperture::<EnergyMeter>("input_1", "output_1");
    }
    #[test]
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<EnergyMeter>()
    }
    #[test]
    fn analyze_wrong() -> OpmResult<()> {
        let mut node = EnergyMeter::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    #[test]
    fn analyze_ok() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        let mut input = LightResult::default();
        let input_data = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_data.clone());
        let result = AnalysisEnergy::analyze(&mut meter, input, &EnergyConfig::default())?;
        assert_eq!(result.get("output_1"), Some(&input_data));
        Ok(())
    }
    #[test]
    fn analyze_apodization_warning() -> OpmResult<()> {
        test_analyze_apodization_warning::<EnergyMeter>()
    }
    #[test]
    fn analyze_inverted() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        let mut input = LightResult::default();
        meter.set_inverted(true)?;
        let input_data = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_data.clone());
        let result = AnalysisEnergy::analyze(&mut meter, input, &EnergyConfig::default())?;
        assert_eq!(result.get("input_1"), Some(&input_data));
        Ok(())
    }
    #[test]
    fn debug() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        assert_eq!(format!("{meter:?}"), "no data");
        let mut input = LightResult::default();
        let input_data = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_data.clone());
        AnalysisEnergy::analyze(&mut meter, input, &EnergyConfig::default())?;
        assert_eq!(format!("{meter:?}"), "Energy: 1 J (Type: IdealEnergyMeter)");
        Ok(())
    }
    #[test]
    fn report() -> OpmResult<()> {
        let mut meter = EnergyMeter::default();
        let Some(report) = meter.node_report("123")? else {
            panic!("Report should not be `None`");
        };
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
        let input_data = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_data.clone());
        AnalysisEnergy::analyze(&mut meter, input, &EnergyConfig::default())?;
        let Some(report) = meter.node_report("123")? else {
            panic!("Report should not be `None`");
        };
        if let Ok(Proptype::Energy(e)) = report.properties().get("Energy") {
            assert_eq!(e, &joule!(1.0));
        } else {
            panic!("could not read Energy property");
        }
        Ok(())
    }
}
