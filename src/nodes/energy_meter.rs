#![warn(missing_docs)]
use crate::lightdata::LightData;
use crate::{
    error::OpossumError,
    optic_node::{Dottable, LightResult, Optical},
    optic_ports::OpticPorts,
};
use std::collections::HashMap;
use std::fmt::Debug;

type Result<T> = std::result::Result<T, OpossumError>;

#[non_exhaustive]
#[derive(Debug, Default, PartialEq, Clone, Copy)]
/// Type of the [`EnergyMeter`]. This is currently not used.
pub enum Metertype {
    /// an ideal energy meter
    #[default]
    IdealEnergyMeter,
    /// an ideal power meter (currently not used)
    IdealPowerMeter,
}
#[derive(Default)]
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
    meter_type: Metertype,
}
impl EnergyMeter {
    /// Creates a new [`EnergyMeter`] of the given [`Metertype`].
    pub fn new(meter_type: Metertype) -> Self {
        EnergyMeter {
            light_data: None,
            meter_type,
        }
    }
    /// Returns the meter type of this [`EnergyMeter`].
    pub fn meter_type(&self) -> Metertype {
        self.meter_type
    }
    /// Sets the meter type of this [`EnergyMeter`].
    pub fn set_meter_type(&mut self, meter_type: Metertype) {
        self.meter_type = meter_type;
    }
}
impl Optical for EnergyMeter {
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
    fn export_data(&self, file_name: &str) {
        if let Some(data) = &self.light_data {
            data.export(file_name)
        }
    }
    fn is_detector(&self) -> bool {
        true
    }
}

impl Debug for EnergyMeter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.light_data {
            Some(data) => write!(f, "{} (Type: {:?})", data, self.meter_type),
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
    use crate::{lightdata::DataEnergy, spectrum::create_he_ne_spectrum, analyzer::AnalyzerType};

    use super::*;
    #[test]
    fn new() {
        let meter = EnergyMeter::new(Metertype::IdealEnergyMeter);
        assert!(meter.light_data.is_none());
        assert_eq!(meter.meter_type, Metertype::IdealEnergyMeter);
    }
    #[test]
    fn default() {
        let meter = EnergyMeter::default();
        assert!(meter.light_data.is_none());
        assert_eq!(meter.meter_type, Metertype::IdealEnergyMeter);
        assert_eq!(meter.node_type(), "energy meter");
        assert_eq!(meter.is_detector(), true);
        assert_eq!(meter.node_color(), "lightblue");
    }
    #[test]
    fn meter_type() {
        let meter = EnergyMeter::new(Metertype::IdealEnergyMeter);
        assert_eq!(meter.meter_type(), Metertype::IdealEnergyMeter);
    }
    #[test]
    fn set_meter_type() {
        let mut meter = EnergyMeter::new(Metertype::IdealEnergyMeter);
        meter.set_meter_type(Metertype::IdealPowerMeter);
        assert_eq!(meter.meter_type, Metertype::IdealPowerMeter);
    }
    #[test]
    fn ports() {
        let meter = EnergyMeter::new(Metertype::IdealEnergyMeter);
        let ports = meter.ports();
        assert_eq!(ports.inputs(), vec!["in1"]);
        assert_eq!(ports.outputs(), vec!["out1"]);
    }
    #[test]
    fn analyze() {
        let mut meter = EnergyMeter::new(Metertype::IdealEnergyMeter);
        let mut input = LightResult::default();
        input.insert(
            "in1".into(),
            Some(LightData::Energy(DataEnergy {
                spectrum: create_he_ne_spectrum(1.0),
            })),
        );
        let result=meter.analyze(input, &AnalyzerType::Energy);
        assert!(result.is_ok());
    }
}
