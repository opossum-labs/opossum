#![warn(missing_docs)]
use serde_derive::{Deserialize, Serialize};
use serde_json::{json, Number};

use crate::dottable::Dottable;
use crate::lightdata::LightData;
use crate::properties::{Properties, Property, Proptype};
use crate::{
    error::OpossumError,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
};
use std::collections::HashMap;
use std::fmt::Debug;

type Result<T> = std::result::Result<T, OpossumError>;

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
/// (ideal) energy / power meter.
///
/// It normally measures the total energy of the incoming light regardless of the wavelength, position, angle, polarization etc...
///
/// ## Optical Ports
///   - Inputs
///     - `in1`
///   - Outputs
///     - `out1`
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere in between arbitrary optic nodes.
pub struct EnergyMeter {
    light_data: Option<LightData>,
    //meter_type: Metertype,
    props: Properties,
}

fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props.set("name", "energy meter".into());
    props.set("inverted", false.into());
    props.set("meter type", Metertype::default().into());
    props
}

impl Default for EnergyMeter {
    fn default() -> Self {
        Self {
            light_data: Default::default(),
            //meter_type: Default::default(),
            props: create_default_props(),
        }
    }
}
impl EnergyMeter {
    /// Creates a new [`EnergyMeter`] of the given [`Metertype`].
    pub fn new(name: &str, meter_type: Metertype) -> Self {
        let mut props = create_default_props();
        props.set("name", name.into());
        props.set("meter type", meter_type.into());
        EnergyMeter {
            light_data: None,
            props,
        }
    }
    /// Returns the meter type of this [`EnergyMeter`].
    pub fn meter_type(&self) -> Metertype {
        let meter_type=self.props.get("meter type").unwrap().prop.clone();
        if let Proptype::Metertype(meter_type)=meter_type {
            meter_type
        } else {
            panic!("wrong data format")
        }
    }
    /// Sets the meter type of this [`EnergyMeter`].
    pub fn set_meter_type(&mut self, meter_type: Metertype) {
        self.props.set("meter type", meter_type.into());
    }
}
impl Optical for EnergyMeter {
    fn set_name(&mut self, name: &str) {
        self.props.set("name", name.into());
    }
    fn name(&self) -> &str {
        if let Some(value) = self.props.get("name") {
            if let Proptype::String(name) = &value.prop {
                return name;
            }
        }
        panic!("wrong format");
    }
    fn node_type(&self) -> &str {
        "energy meter"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("in1").unwrap();
        ports.add_output("out1").unwrap();
        ports
    }
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> Result<LightResult> {
        if let Some(data) = incoming_data.get("in1") {
            self.light_data = data.clone();
            Ok(HashMap::from([("out1".into(), data.clone())]))
        } else {
            Ok(HashMap::from([("out2".into(), None)]))
        }
    }
    fn is_detector(&self) -> bool {
        true
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Property) -> Result<()> {
        if self.props.set(name, prop).is_none() {
            Err(OpossumError::Other("property not defined".into()))
        } else {
            Ok(())
        }
    }
    fn report(&self) -> serde_json::Value {
        let data = &self.light_data;
        let mut energy_data = serde_json::Value::Null;
        if let Some(LightData::Energy(e)) = data {
            energy_data =
                serde_json::Value::Number(Number::from_f64(e.spectrum.total_energy()).unwrap())
        }
        json!({"type": self.node_type(),
        "name": self.name(),
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
    use crate::{analyzer::AnalyzerType, lightdata::DataEnergy, spectrum::create_he_ne_spectrum};

    use super::*;
    #[test]
    fn new() {
        let meter = EnergyMeter::new("test", Metertype::IdealEnergyMeter);
        assert!(meter.light_data.is_none());
        assert_eq!(meter.meter_type(), Metertype::IdealEnergyMeter);
    }
    #[test]
    fn default() {
        let meter = EnergyMeter::default();
        assert!(meter.light_data.is_none());
        assert_eq!(meter.meter_type(), Metertype::IdealEnergyMeter);
        assert_eq!(meter.node_type(), "energy meter");
        assert_eq!(meter.is_detector(), true);
        assert_eq!(meter.node_color(), "whitesmoke");
        assert_eq!(meter.name(), "energy meter");
    }
    #[test]
    fn meter_type() {
        let meter = EnergyMeter::new("test", Metertype::IdealEnergyMeter);
        assert_eq!(meter.meter_type(), Metertype::IdealEnergyMeter);
    }
    #[test]
    fn set_meter_type() {
        let mut meter = EnergyMeter::new("test", Metertype::IdealEnergyMeter);
        meter.set_meter_type(Metertype::IdealPowerMeter);
        assert_eq!(meter.meter_type(), Metertype::IdealPowerMeter);
    }
    #[test]
    fn ports() {
        let meter = EnergyMeter::new("test", Metertype::IdealEnergyMeter);
        let ports = meter.ports();
        assert_eq!(ports.inputs(), vec!["in1"]);
        assert_eq!(ports.outputs(), vec!["out1"]);
    }
    #[test]
    fn analyze() {
        let mut meter = EnergyMeter::new("test", Metertype::IdealEnergyMeter);
        let mut input = LightResult::default();
        input.insert(
            "in1".into(),
            Some(LightData::Energy(DataEnergy {
                spectrum: create_he_ne_spectrum(1.0),
            })),
        );
        let result = meter.analyze(input, &AnalyzerType::Energy);
        assert!(result.is_ok());
    }
}
