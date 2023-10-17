#![warn(missing_docs)]
use serde_derive::{Deserialize, Serialize};

use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::{DataEnergy, LightData};
use crate::optic_ports::OpticPorts;
use crate::optical::{LightResult, Optical};
use crate::properties::{Properties, Proptype};
use crate::spectrum::Spectrum;
use std::collections::HashMap;

/// Config data for an [`IdealFilter`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FilterType {
    /// a fixed (wavelength-independant) transmission value. Must be between 0.0 and 1.0
    Constant(f64),
    /// filter based on given transmission spectrum.
    Spectrum(Spectrum),
}
#[derive(Debug)]
/// An ideal filter with given transmission or optical density.
///
/// ## Optical Ports
///   - Inputs
///     - `front`
///   - Outputs
///     - `rear`
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `filter type`
pub struct IdealFilter {
    props: Properties,
}

fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props.create("name", "ideal filter".into()).unwrap();
    props.create("inverted", false.into()).unwrap();
    props
        .create("filter type", FilterType::Constant(1.0).into())
        .unwrap();
    props
}
impl Default for IdealFilter {
    /// Create an ideal filter node with a transmission of 100%.
    fn default() -> Self {
        Self {
            props: create_default_props(),
        }
    }
}
impl IdealFilter {
    /// Creates a new [`IdealFilter`] with a given [`FilterType`].
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError::Other`] if the filter type is
    /// [`FilterType::Constant`] and the transmission factor is outside the interval [0.0; 1.0].
    pub fn new(name: &str, filter_type: FilterType) -> OpmResult<Self> {
        if let FilterType::Constant(transmission) = filter_type {
            if !(0.0..=1.0).contains(&transmission) {
                return Err(OpossumError::Other(
                    "attenuation must be in interval [0.0; 1.0]".into(),
                ));
            }
        }
        let mut props = create_default_props();
        props.set("filter type", filter_type.into())?;
        props.set("name", name.into())?;
        Ok(Self { props })
    }
    /// Returns the filter type of this [`IdealFilter`].
    pub fn filter_type(&self) -> FilterType {
        if let Proptype::FilterType(filter_type) = self.props.get("filter type").unwrap() {
            filter_type.clone()
        } else {
            panic!("wrong data type")
        }
    }
    /// Sets a constant transmission value for this [`IdealFilter`].
    ///
    /// This implicitly sets the filter type to [`FilterType::Constant`].
    /// # Errors
    ///
    /// This function will return an error if a transmission factor > 1.0 is given (This would be an amplifiying filter :-) ).
    pub fn set_transmission(&mut self, transmission: f64) -> OpmResult<()> {
        if (0.0..=1.0).contains(&transmission) {
            self.props
                .set("filter type", FilterType::Constant(transmission).into())?;
            Ok(())
        } else {
            Err(OpossumError::Other(
                "attenuation must be in interval [0.0; 1.0]".into(),
            ))
        }
    }
    /// Sets the transmission of this [`IdealFilter`] expressed as optical density.
    ///
    /// This implicitly sets the filter type to [`FilterType::Constant`].
    /// # Errors
    ///
    /// This function will return an error if an optical density < 0.0 was given.
    pub fn set_optical_density(&mut self, density: f64) -> OpmResult<()> {
        if density >= 0.0 {
            self.props.set(
                "filter type",
                FilterType::Constant(f64::powf(10.0, -1.0 * density)).into(),
            )?;
            Ok(())
        } else {
            Err(OpossumError::Other("optical densitiy must be >=0".into()))
        }
    }
    /// Returns the transmission factor of this [`IdealFilter`] expressed as optical density for the [`FilterType::Constant`].
    ///
    /// This functions `None` if the filter type is not [`FilterType::Constant`].
    pub fn optical_density(&self) -> Option<f64> {
        match self.filter_type() {
            FilterType::Constant(t) => Some(-1.0 * f64::log10(t)),
            _ => None,
        }
    }
    fn analyze_energy(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let (mut src, mut target) = ("front", "rear");
        if self.inverted() {
            (src, target) = (target, src)
        }
        let input = incoming_data.get(src);
        if let Some(Some(input)) = input {
            match input {
                LightData::Energy(e) => {
                    let mut out_spec = e.spectrum.clone();
                    match &self.filter_type() {
                        FilterType::Constant(t) => {
                            if out_spec.scale_vertical(*t).is_ok() {
                                let light_data =
                                    Some(LightData::Energy(DataEnergy { spectrum: out_spec }));
                                return Ok(HashMap::from([(target.into(), light_data)]));
                            }
                        }
                        FilterType::Spectrum(s) => {
                            out_spec.filter(s);
                            let light_data =
                                Some(LightData::Energy(DataEnergy { spectrum: out_spec }));
                            return Ok(HashMap::from([(target.into(), light_data)]));
                        }
                    }
                }
                _ => return Err(OpossumError::Analysis("expected energy value".into())),
            }
        }
        Err(OpossumError::Analysis("no data on input port".into()))
    }
}

impl Optical for IdealFilter {
    fn name(&self) -> &str {
        if let Proptype::String(name) = &self.props.get("name").unwrap() {
            name
        } else {
            self.node_type()
        }
    }
    fn node_type(&self) -> &str {
        "ideal filter"
    }
    fn inverted(&self) -> bool {
        self.properties().get_bool("inverted").unwrap().unwrap()
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("front").unwrap();
        ports.add_output("rear").unwrap();
        if self.inverted() {
            ports.set_inverted(true);
        }
        ports
    }
    fn analyze(
        &mut self,
        incoming_data: crate::optical::LightResult,
        analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> OpmResult<crate::optical::LightResult> {
        match analyzer_type {
            AnalyzerType::Energy => self.analyze_energy(incoming_data),
            _ => Err(OpossumError::Analysis(
                "analysis type not yet implemented".into(),
            )),
        }
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.props.set(name, prop)
    }
}

impl Dottable for IdealFilter {
    fn node_color(&self) -> &str {
        "darkgray"
    }
}
#[cfg(test)]
mod test {
    use crate::spectrum::create_he_ne_spectrum;

    use super::*;
    #[test]
    fn default() {
        let node = IdealFilter::default();
        assert_eq!(node.filter_type(), FilterType::Constant(1.0));
        assert_eq!(node.name(), "ideal filter");
        assert_eq!(node.node_type(), "ideal filter");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "darkgray");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let node = IdealFilter::new("test", FilterType::Constant(0.8)).unwrap();
        assert_eq!(node.name(), "test");
        assert_eq!(node.filter_type(), FilterType::Constant(0.8));
    }
    #[test]
    fn inverted() {
        let mut node = IdealFilter::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.inverted(), true)
    }
    #[test]
    fn ports() {
        let node = IdealFilter::default();
        assert_eq!(node.ports().inputs(), vec!["front"]);
        assert_eq!(node.ports().outputs(), vec!["rear"]);
    }
    #[test]
    fn ports_inverted() {
        let mut node = IdealFilter::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.ports().inputs(), vec!["rear"]);
        assert_eq!(node.ports().outputs(), vec!["front"]);
    }
    #[test]
    fn analyze_ok() {
        let mut node = IdealFilter::new("test", FilterType::Constant(0.5)).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("front".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("rear".into()));
        assert_eq!(output.len(), 1);
        let output = output.get("rear".into()).unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        let expected_output_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(0.5),
        });
        assert_eq!(output, expected_output_light);
    }
    #[test]
    fn analyze_wrong() {
        let mut node = IdealFilter::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("rear".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_err());
    }
    #[test]
    fn analyze_inverse() {
        let mut node = IdealFilter::new("test", FilterType::Constant(0.5)).unwrap();
        node.set_property("inverted", true.into()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("rear".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("front".into()));
        assert_eq!(output.len(), 1);
        let output = output.get("front".into()).unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        let expected_output_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(0.5),
        });
        assert_eq!(output, expected_output_light);
    }
}
