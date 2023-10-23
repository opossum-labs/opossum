#![warn(missing_docs)]
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Number};

use crate::dottable::Dottable;
use crate::error::OpmResult;
use crate::lightdata::LightData;
use crate::properties::{PropCondition, Properties, Proptype};
use crate::{
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
};
use std::collections::HashMap;
use std::fmt::Debug;

#[non_exhaustive]
#[derive(Debug, Default, PartialEq, Clone, Copy, Serialize, Deserialize)]
/// Type of the [`EnergyMeter`]. This is currently not used.
pub enum Metertype {
    /// an ideal energy meter
    #[default]
    IdealEnergyMeter,
    /// an ideal power meter (currently not used)
    IdealPowerMeter,
}
impl From<Metertype> for Proptype {
    fn from(value: Metertype) -> Self {
        Proptype::Metertype(value)
    }
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
pub struct EnergyMeter {
    light_data: Option<LightData>,
    props: Properties,
}

fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props
        .create(
            "name",
            "name of the energy meter",
            Some(vec![PropCondition::NonEmptyString]),
            "energy meter".into(),
        )
        .unwrap();
    props
        .create(
            "node_type",
            "specific optical type of this node",
            Some(vec![PropCondition::NonEmptyString]),
            "energy meter".into(),
        )
        .unwrap();
    props
        .create("inverted", "inverse propagation?", None, false.into())
        .unwrap();
    props
        .create(
            "meter type",
            "model type of the meter",
            None,
            Metertype::default().into(),
        )
        .unwrap();
    props
}

impl Default for EnergyMeter {
    fn default() -> Self {
        Self {
            light_data: None,
            props: create_default_props(),
        }
    }
}
impl EnergyMeter {
    /// Creates a new [`EnergyMeter`] of the given [`Metertype`].
    pub fn new(name: &str, meter_type: Metertype) -> Self {
        let mut props = create_default_props();
        props.set("name", name.into()).unwrap();
        props.set("meter type", meter_type.into()).unwrap();
        EnergyMeter {
            props,
            ..Default::default()
        }
    }
    /// Returns the meter type of this [`EnergyMeter`].
    pub fn meter_type(&self) -> Metertype {
        if let Proptype::Metertype(meter_type) = self.props.get("meter type").unwrap() {
            *meter_type
        } else {
            panic!("wrong data format")
        }
    }
    /// Sets the meter type of this [`EnergyMeter`].
    pub fn set_meter_type(&mut self, meter_type: Metertype) {
        self.props.set("meter type", meter_type.into()).unwrap();
    }
}
impl Optical for EnergyMeter {
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("in1").unwrap();
        ports.add_output("out1").unwrap();
        if self.properties().inverted() {
            ports.set_inverted(true);
        }
        ports
    }
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (src, target) = if self.properties().inverted() {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let data = incoming_data.get(src).unwrap_or(&None);
        self.light_data = data.clone();
        Ok(HashMap::from([(target.into(), data.clone())]))
    }
    fn is_detector(&self) -> bool {
        true
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.props.set(name, prop)
    }
    fn report(&self) -> serde_json::Value {
        let data = &self.light_data;
        let mut energy_data = serde_json::Value::Null;
        if let Some(LightData::Energy(e)) = data {
            energy_data =
                serde_json::Value::Number(Number::from_f64(e.spectrum.total_energy()).unwrap())
        }
        json!({"type": self.properties().node_type().unwrap(),
        "name": self.properties().name().unwrap(),
        "energy": energy_data})
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
impl Dottable for EnergyMeter {
    fn node_color(&self) -> &str {
        "whitesmoke"
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{analyzer::AnalyzerType, lightdata::DataEnergy, spectrum::create_he_ne_spectrum};
    #[test]
    fn default() {
        let node = EnergyMeter::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.meter_type(), Metertype::IdealEnergyMeter);
        assert_eq!(node.properties().name().unwrap(), "energy meter");
        assert_eq!(node.properties().node_type().unwrap(), "energy meter");
        assert_eq!(node.is_detector(), true);
        assert_eq!(node.properties().inverted(), false);
        assert_eq!(node.node_color(), "whitesmoke");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let meter = EnergyMeter::new("test", Metertype::IdealPowerMeter);
        assert!(meter.light_data.is_none());
        assert_eq!(meter.meter_type(), Metertype::IdealPowerMeter);
        assert_eq!(meter.properties().name().unwrap(), "test");
    }
    #[test]
    fn inverted() {
        let mut meter = EnergyMeter::new("test", Metertype::IdealPowerMeter);
        meter.set_property("inverted", true.into()).unwrap();
        assert_eq!(meter.properties().inverted(), true);
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
        assert_eq!(meter.ports().inputs(), vec!["in1"]);
        assert_eq!(meter.ports().outputs(), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = EnergyMeter::default();
        meter.set_property("inverted", true.into()).unwrap();
        assert_eq!(meter.ports().inputs(), vec!["out1"]);
        assert_eq!(meter.ports().outputs(), vec!["in1"]);
    }
    #[test]
    fn analyze() {
        let mut meter = EnergyMeter::default();
        let mut input = LightResult::default();
        let input_data = Some(LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        }));
        input.insert("in1".into(), input_data.clone());
        let result = meter.analyze(input, &AnalyzerType::Energy);
        assert!(result.is_ok());
        assert!(result.clone().unwrap().contains_key("out1"));
        assert_eq!(result.unwrap().get("out1").unwrap(), &input_data);
    }
    #[test]
    fn analyze_inverted() {
        let mut meter = EnergyMeter::default();
        let mut input = LightResult::default();
        meter.set_property("inverted", true.into()).unwrap();
        let input_data = Some(LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        }));
        input.insert("out1".into(), input_data.clone());
        let result = meter.analyze(input, &AnalyzerType::Energy);
        assert!(result.is_ok());
        assert!(result.clone().unwrap().contains_key("in1"));
        assert_eq!(result.unwrap().get("in1").unwrap(), &input_data);
    }
}
