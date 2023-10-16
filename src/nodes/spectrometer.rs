#![warn(missing_docs)]
use serde_derive::{Deserialize, Serialize};
use serde_json::json;
use uom::si::length::nanometer;

use crate::dottable::Dottable;
use crate::error::OpmResult;
use crate::lightdata::LightData;
use crate::properties::{Properties, Property, Proptype};
use crate::{
    error::OpossumError,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
};
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};

#[non_exhaustive]
#[derive(Debug, Default, PartialEq, Clone, Copy, Serialize, Deserialize)]
/// Type of the [`Spectrometer`]. This is currently not used.
pub enum SpectrometerType {
    /// an ideal energy meter
    #[default]
    IdealSpectrometer,
    /// Ocean Optics HR2000
    HR2000,
}
/// (ideal) spectrometer
///
/// It normally measures / displays the spectrum of the incoming light.
///
/// ## Optical Ports
///   - Inputs
///     - `in1`
///   - Outputs
///     - `out1`
///
/// ## Properties
///   - `name`
///   - `spectrometer type
/// `
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere within the optical setup.
pub struct Spectrometer {
    light_data: Option<LightData>,
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props.set("name", "spectrometer".into());
    props.set(
        "spectrometer type",
        SpectrometerType::IdealSpectrometer.into(),
    );
    props.set("inverted", false.into());
    props
}
impl Default for Spectrometer {
    /// create an ideal spectrometer.
    fn default() -> Self {
        Self {
            light_data: None,
            props: create_default_props(),
        }
    }
}
impl Spectrometer {
    /// Creates a new [`Spectrometer`] of the given [`SpectrometerType`].
    pub fn new(name: &str, spectrometer_type: SpectrometerType) -> Self {
        let mut props = create_default_props();
        props.set("spectrometer type", spectrometer_type.into());
        props.set("name", name.into());
        Spectrometer {
            props,
            ..Default::default()
        }
    }
    /// Returns the meter type of this [`Spectrometer`].
    pub fn spectrometer_type(&self) -> SpectrometerType {
        let meter_type = self.props.get("spectrometer type").unwrap().prop.clone();
        if let Proptype::SpectrometerType(meter_type) = meter_type {
            meter_type
        } else {
            panic!("wrong data format")
        }
    }
    /// Sets the meter type of this [`Spectrometer`].
    pub fn set_spectrometer_type(&mut self, meter_type: SpectrometerType) {
        self.props.set("spectrometer type", meter_type.into());
    }
}
impl Optical for Spectrometer {
    fn name(&self) -> &str {
        if let Proptype::String(name) = &self.props.get("name").unwrap().prop {
            name
        } else {
            self.node_type()
        }
    }
    fn node_type(&self) -> &str {
        "spectrometer"
    }
    fn inverted(&self) -> bool {
        self.properties().get_bool("inverted").unwrap().unwrap()
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
    ) -> OpmResult<LightResult> {
        let (src, target) = if self.inverted() {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let data = incoming_data.get(src).unwrap_or(&None);
        self.light_data = data.clone();
        Ok(HashMap::from([(target.into(), data.clone())]))
    }
    fn export_data(&self, report_dir: &Path) {
        if let Some(data) = &self.light_data {
            let mut file_path = PathBuf::from(report_dir);
            file_path.push(format!("spectrum_{}.svg", self.name()));
            data.export(&file_path)
        }
    }
    fn is_detector(&self) -> bool {
        true
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Property) -> OpmResult<()> {
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
            energy_data = e.spectrum.to_json();
        }
        json!({"type": self.node_type(),
        "name": self.name(),
        "energy": energy_data})
    }
}

impl Debug for Spectrometer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.light_data {
            Some(data) => match data {
                LightData::Energy(data_energy) => {
                    let spectrum_range = data_energy.spectrum.range();
                    write!(
                        f,
                        "Spectrum {:.3} - {:.3} nm (Type: {:?})",
                        spectrum_range.start.get::<nanometer>(),
                        spectrum_range.end.get::<nanometer>(),
                        self.spectrometer_type()
                    )
                }
                _ => write!(f, "no spectrum data to display"),
            },
            None => write!(f, "no data"),
        }
    }
}
impl Dottable for Spectrometer {
    fn node_color(&self) -> &str {
        "lightseagreen"
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{analyzer::AnalyzerType, lightdata::DataEnergy, spectrum::create_he_ne_spectrum};
    #[test]
    fn default() {
        let node = Spectrometer::default();
        assert!(node.light_data.is_none());
        assert_eq!(
            node.spectrometer_type(),
            SpectrometerType::IdealSpectrometer
        );
        assert_eq!(node.name(), "spectrometer");
        assert_eq!(node.node_type(), "spectrometer");
        assert_eq!(node.is_detector(), true);
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "lightseagreen");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let meter = Spectrometer::new("test", SpectrometerType::HR2000);
        assert_eq!(meter.name(), "test");
        assert!(meter.light_data.is_none());
        assert_eq!(meter.spectrometer_type(), SpectrometerType::HR2000);
    }
    #[test]
    fn set_meter_type() {
        let mut meter = Spectrometer::new("test", SpectrometerType::IdealSpectrometer);
        meter.set_spectrometer_type(SpectrometerType::HR2000);
        assert_eq!(meter.spectrometer_type(), SpectrometerType::HR2000);
    }
    #[test]
    fn ports() {
        let meter = Spectrometer::default();
        assert_eq!(meter.ports().inputs(), vec!["in1"]);
        assert_eq!(meter.ports().outputs(), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        todo!()
    }
    #[test]
    fn inverted() {
        let mut node = Spectrometer::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.inverted(), true)
    }
    #[test]
    fn analyze_ok() {
        let mut node = Spectrometer::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("in1".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("out1".into()));
        assert_eq!(output.len(), 1);
        let output = output.get("out1".into()).unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
    #[test]
    fn analyze_wrong() {
        let mut node = Spectrometer::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("wrong".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        let output = output.get("out1".into()).unwrap();
        assert!(output.is_none());
    }
    #[test]
    fn analyze_inverse() {
        let mut node = Spectrometer::default();
        node.set_property("inverted", true.into()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("out1".into(), Some(input_light.clone()));

        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("in1".into()));
        assert_eq!(output.len(), 1);
        let output = output.get("in1".into()).unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
}
