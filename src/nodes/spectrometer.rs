use uom::si::length::nanometer;

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
pub enum SpectrometerType {
    /// an ideal energy meter
    #[default]
    IdealSpectrometer,
    /// Ocean Optics HR2000
    HR2000,
}
#[derive(Default)]
/// (ideal) spectrometer
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
pub struct Spectrometer {
    light_data: Option<LightData>,
    spectrometer_type: SpectrometerType,
}
impl Spectrometer {
    /// Creates a new [`Spectrometer`] of the given [`SpectrometerType`].
    pub fn new(spectrometer_type: SpectrometerType) -> Self {
        Spectrometer {
            light_data: None,
            spectrometer_type,
        }
    }
    /// Returns the meter type of this [`Spectrometer`].
    pub fn spectrometer_type(&self) -> SpectrometerType {
        self.spectrometer_type
    }
    /// Sets the meter type of this [`EnergyMeter`].
    pub fn set_spectrometer_type(&mut self, meter_type: SpectrometerType) {
        self.spectrometer_type = meter_type;
    }
}
impl Optical for Spectrometer {
    fn node_type(&self) -> &str {
        "spectrometer"
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
                        self.spectrometer_type
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
    use crate::{analyzer::AnalyzerType, lightdata::DataEnergy, spectrum::create_he_ne_spectrum};

    use super::*;
    #[test]
    fn new() {
        let meter = Spectrometer::new(SpectrometerType::IdealSpectrometer);
        assert!(meter.light_data.is_none());
        assert_eq!(meter.spectrometer_type, SpectrometerType::IdealSpectrometer);
    }
    #[test]
    fn default() {
        let meter = Spectrometer::default();
        assert!(meter.light_data.is_none());
        assert_eq!(meter.spectrometer_type, SpectrometerType::IdealSpectrometer);
        assert_eq!(meter.node_type(), "spectrometer");
        assert_eq!(meter.is_detector(), true);
        assert_eq!(meter.node_color(), "lightseagreen");
    }
    #[test]
    fn meter_type() {
        let meter = Spectrometer::new(SpectrometerType::IdealSpectrometer);
        assert_eq!(
            meter.spectrometer_type(),
            SpectrometerType::IdealSpectrometer
        );
    }
    #[test]
    fn set_meter_type() {
        let mut meter = Spectrometer::new(SpectrometerType::IdealSpectrometer);
        meter.set_spectrometer_type(SpectrometerType::HR2000);
        assert_eq!(meter.spectrometer_type, SpectrometerType::HR2000);
    }
    #[test]
    fn ports() {
        let meter = Spectrometer::default();
        let ports = meter.ports();
        assert_eq!(ports.inputs(), vec!["in1"]);
        assert_eq!(ports.outputs(), vec!["out1"]);
    }
    #[test]
    fn analyze() {
        let mut meter = Spectrometer::default();
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
