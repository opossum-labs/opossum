#![warn(missing_docs)]
use serde_derive::{Deserialize, Serialize};
use uom::si::length::nanometer;

use crate::dottable::Dottable;
use crate::error::OpmResult;
use crate::lightdata::LightData;
use crate::properties::{Properties, Proptype};
use crate::reporter::{NodeReport, PdfReportable};
use crate::{
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
};
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};

#[non_exhaustive]
#[derive(Debug, Default, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
/// Type of the [`Spectrometer`]. This is currently not used.
pub enum SpectrometerType {
    /// an ideal energy meter
    #[default]
    Ideal,
    /// Ocean Optics HR2000
    HR2000,
}
impl From<SpectrometerType> for Proptype {
    fn from(value: SpectrometerType) -> Self {
        Self::SpectrometerType(value)
    }
}
impl PdfReportable for SpectrometerType {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let element = match self {
            Self::Ideal => genpdf::elements::Text::new("ideal spectrometer"),
            Self::HR2000 => genpdf::elements::Text::new("Ocean Optics HR2000"),
        };
        let mut l = genpdf::elements::LinearLayout::vertical();
        l.push(element);
        Ok(l)
    }
}
/// An (ideal) spectrometer
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
    let mut props = Properties::new("spectrometer", "spectrometer");
    props
        .create(
            "spectrometer type",
            "model type of the spectrometer",
            None,
            SpectrometerType::Ideal.into(),
        )
        .unwrap();
    let mut ports = OpticPorts::new();
    ports.create_input("in1").unwrap();
    ports.create_output("out1").unwrap();
    props.set("apertures", ports.into()).unwrap();
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
    /// # Attributes
    /// * `name`: name of the  [`Spectrometer`]
    /// * `spectrometer_type`: [`SpectrometerType`] of the  [`Spectrometer`]
    ///
    /// # Panics
    /// This function panics if
    /// - the property "spectrometer" type can not be set.
    /// - the property "name" type can not be set.
    #[must_use]
    pub fn new(name: &str, spectrometer_type: SpectrometerType) -> Self {
        let mut props = create_default_props();
        props
            .set("spectrometer type", spectrometer_type.into())
            .unwrap();
        props.set("name", name.into()).unwrap();
        Self {
            props,
            ..Default::default()
        }
    }
    /// Returns the meter type of this [`Spectrometer`].
    ///
    /// # Panics
    /// This function panics if
    /// - the property "spectrometer type" is not defined or
    /// - the meter type has the wrong data format
    #[must_use]
    pub fn spectrometer_type(&self) -> SpectrometerType {
        let meter_type = self.props.get("spectrometer type").unwrap().clone();
        if let Proptype::SpectrometerType(meter_type) = meter_type {
            meter_type
        } else {
            panic!("wrong data format")
        }
    }
    /// Sets the meter type of this [`Spectrometer`].
    /// /// # Attributes
    /// * `meter_type`: [`SpectrometerType`] of the  [`Spectrometer`]
    ///
    /// # Errors
    /// This function returns an error if
    /// - the property "spectrometer type" type can not be set.
    pub fn set_spectrometer_type(&mut self, meter_type: SpectrometerType) -> OpmResult<()> {
        self.props.set("spectrometer type", meter_type.into())?;
        Ok(())
    }
}
impl Optical for Spectrometer {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (src, target) = if self.properties().inverted()? {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let data = incoming_data.get(src).unwrap_or(&None);
        self.light_data = data.clone();
        Ok(HashMap::from([(target.into(), data.clone())]))
    }
    fn export_data(&self, report_dir: &Path) -> OpmResult<()> {
        if let Some(data) = &self.light_data {
            let mut file_path = PathBuf::from(report_dir);
            file_path.push(format!("spectrum_{}.svg", self.properties().name()?));
            data.export(&file_path)
        } else {
            Ok(())
        }
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
    fn report(&self) -> Option<NodeReport> {
        let mut props = Properties::default();
        let data = &self.light_data;
        if let Some(LightData::Energy(e)) = data {
            props
                .create(
                    "Spectrum",
                    "Output spectrum",
                    None,
                    e.spectrum.clone().into(),
                )
                .unwrap();
            props
                .create(
                    "Model",
                    "Spectrometer model",
                    None,
                    self.props.get("spectrometer type").unwrap().clone(),
                )
                .unwrap();
        }
        Some(NodeReport::new(
            self.properties().node_type().unwrap(),
            self.properties().name().unwrap(),
            props,
        ))
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
    use crate::{
        analyzer::AnalyzerType,
        lightdata::DataEnergy,
        spectrum::{create_he_ne_spec, create_visible_spec},
    };
    #[test]
    fn debug() {
        let mut node = Spectrometer::default();
        assert_eq!(format!("{:?}", node), "no data");
        let mut input = LightResult::default();
        input.insert("in1".into(), Some(LightData::Fourier));
        let _ = node.analyze(input, &AnalyzerType::Energy);
        assert_eq!(format!("{:?}", node), "no spectrum data to display");
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_visible_spec(),
        });
        input.insert("in1".into(), Some(input_light.clone()));
        let _ = node.analyze(input, &AnalyzerType::Energy);
        assert_eq!(
            format!("{:?}", node),
            "Spectrum 380.000 - 749.900 nm (Type: Ideal)"
        );
    }
    #[test]
    fn default() {
        let node = Spectrometer::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.spectrometer_type(), SpectrometerType::Ideal);
        assert_eq!(node.properties().name().unwrap(), "spectrometer");
        assert_eq!(node.properties().node_type().unwrap(), "spectrometer");
        assert_eq!(node.is_detector(), true);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "lightseagreen");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let meter = Spectrometer::new("test", SpectrometerType::HR2000);
        assert_eq!(meter.properties().name().unwrap(), "test");
        assert!(meter.light_data.is_none());
        assert_eq!(meter.spectrometer_type(), SpectrometerType::HR2000);
    }
    #[test]
    fn set_meter_type() {
        let mut meter = Spectrometer::new("test", SpectrometerType::Ideal);
        meter
            .set_spectrometer_type(SpectrometerType::HR2000)
            .unwrap();
        assert_eq!(meter.spectrometer_type(), SpectrometerType::HR2000);
    }
    #[test]
    fn ports() {
        let meter = Spectrometer::default();
        assert_eq!(meter.ports().input_names(), vec!["in1"]);
        assert_eq!(meter.ports().output_names(), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut meter = Spectrometer::default();
        meter.set_property("inverted", true.into()).unwrap();
        assert_eq!(meter.ports().input_names(), vec!["out1"]);
        assert_eq!(meter.ports().output_names(), vec!["in1"]);
    }
    #[test]
    fn inverted() {
        let mut node = Spectrometer::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.properties().inverted().unwrap(), true)
    }
    #[test]
    fn analyze_ok() {
        let mut node = Spectrometer::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("in1".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1").unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
    #[test]
    fn analyze_wrong() {
        let mut node = Spectrometer::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("wrong".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        let output = output.get("out1").unwrap();
        assert!(output.is_none());
    }
    #[test]
    fn analyze_inverse() {
        let mut node = Spectrometer::default();
        node.set_property("inverted", true.into()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("out1".into(), Some(input_light.clone()));

        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("in1"));
        assert_eq!(output.len(), 1);
        let output = output.get("in1").unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
}
