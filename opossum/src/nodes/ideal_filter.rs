#![warn(missing_docs)]
use serde_derive::{Deserialize, Serialize};

use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::LightData;
use crate::optic_ports::OpticPorts;
use crate::optical::{LightResult, Optical};
use crate::properties::{Properties, Proptype};
use crate::reporter::PdfReportable;
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
impl From<FilterType> for Proptype {
    fn from(value: FilterType) -> Self {
        Self::FilterType(value)
    }
}
impl PdfReportable for FilterType {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let mut l = genpdf::elements::LinearLayout::vertical();
        match self {
            Self::Constant(value) => l.push(genpdf::elements::Text::new(format!(
                "fixed attenuation: {value}"
            ))),
            Self::Spectrum(spectrum) => {
                l.push(genpdf::elements::Text::new("transmission spectrum"));
                l.push(spectrum.pdf_report()?);
            }
        };
        Ok(l)
    }
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
    let mut props = Properties::new("ideal filter", "ideal filter");
    props
        .create(
            "filter type",
            "used filter algorithm",
            None,
            FilterType::Constant(1.0).into(),
        )
        .unwrap();
    let mut ports = OpticPorts::new();
    ports.create_input("front").unwrap();
    ports.create_output("rear").unwrap();
    props.set("apertures", ports.into()).unwrap();
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
    ///
    /// # Panics
    /// Panics if the wrong data type is stored in the filter-type properties
    #[must_use]
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
    #[must_use]
    pub fn optical_density(&self) -> Option<f64> {
        match self.filter_type() {
            FilterType::Constant(t) => Some(-1.0 * f64::log10(t)),
            FilterType::Spectrum(_) => None,
        }
    }
    fn analyze_energy(&mut self, incoming_data: &LightResult) -> OpmResult<LightResult> {
        let (mut src, mut target) = ("front", "rear");
        if self.properties().inverted()? {
            (src, target) = (target, src);
        }
        let input = incoming_data.get(src);
        if let Some(Some(input)) = input {
            match input {
                LightData::Energy(e) => {
                    let mut new_data = e.clone();
                    new_data.filter(&self.filter_type())?;
                    let light_data = Some(LightData::Energy(new_data));
                    return Ok(HashMap::from([(target.into(), light_data)]));
                }
                LightData::Geometric(r) => {
                    let mut new_rays = r.clone();
                    new_rays.filter_energy(&self.filter_type())?;
                    let light_data = Some(LightData::Geometric(new_rays));
                    return Ok(HashMap::from([(target.into(), light_data)]));
                }
                LightData::Fourier => {
                    return Err(OpossumError::Analysis(
                        "Ideal filter: expected LightData::Energy or LightData::Geometric".into(),
                    ))
                }
            }
        }
        Err(OpossumError::Analysis("no data on input port".into()))
    }
}

impl Optical for IdealFilter {
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        if self.properties().inverted().unwrap() {
            ports.set_inverted(true);
        }
        ports
    }
    fn analyze(
        &mut self,
        incoming_data: crate::optical::LightResult,
        _analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> OpmResult<crate::optical::LightResult> {
        self.analyze_energy(&incoming_data)
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
    use approx::assert_abs_diff_eq;
    use uom::si::{
        energy::joule,
        f64::{Energy, Length},
        length::{millimeter, nanometer},
    };

    use crate::{
        analyzer::{AnalyzerType, RayTraceConfig},
        lightdata::DataEnergy,
        rays::{DistributionStrategy, Rays},
        spectrum::create_he_ne_spec,
    };

    use super::*;
    #[test]
    fn default() {
        let node = IdealFilter::default();
        assert_eq!(node.filter_type(), FilterType::Constant(1.0));
        assert_eq!(node.properties().name().unwrap(), "ideal filter");
        assert_eq!(node.properties().node_type().unwrap(), "ideal filter");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "darkgray");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let node = IdealFilter::new("test", FilterType::Constant(0.8)).unwrap();
        assert_eq!(node.properties().name().unwrap(), "test");
        assert_eq!(node.filter_type(), FilterType::Constant(0.8));
    }
    #[test]
    fn inverted() {
        let mut node = IdealFilter::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.properties().inverted().unwrap(), true)
    }
    #[test]
    fn ports() {
        let node = IdealFilter::default();
        assert_eq!(node.ports().input_names(), vec!["front"]);
        assert_eq!(node.ports().output_names(), vec!["rear"]);
    }
    #[test]
    fn ports_inverted() {
        let mut node = IdealFilter::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.ports().input_names(), vec!["rear"]);
        assert_eq!(node.ports().output_names(), vec!["front"]);
    }
    #[test]
    fn analyze_energy_ok() {
        let mut node = IdealFilter::new("test", FilterType::Constant(0.5)).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("front".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("rear"));
        assert_eq!(output.len(), 1);
        let output = output.get("rear").unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        let expected_output_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(0.5).unwrap(),
        });
        assert_eq!(output, expected_output_light);
    }
    #[test]
    fn analyzer_geometric_fixed() {
        let mut node = IdealFilter::new("test", FilterType::Constant(0.3)).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(
            Rays::new_uniform_collimated(
                Length::new::<millimeter>(5.0),
                Length::new::<nanometer>(1054.0),
                Energy::new::<joule>(1.0),
                &DistributionStrategy::Hexapolar(1),
            )
            .unwrap(),
        );
        input.insert("front".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::RayTrace(RayTraceConfig::default()));
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("rear"));
        assert_eq!(output.len(), 1);
        let output = output.get("rear").unwrap();
        assert!(output.is_some());
        if let LightData::Geometric(output) = output.clone().unwrap() {
            assert_abs_diff_eq!(output.total_energy().get::<joule>(), 0.3);
        } else {
            panic!("wrong data LightData format")
        }
    }
    #[test]
    fn analyze_wrong() {
        let mut node = IdealFilter::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
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
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("rear".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("front"));
        assert_eq!(output.len(), 1);
        let output = output.get("front").unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        let expected_output_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(0.5).unwrap(),
        });
        assert_eq!(output, expected_output_light);
    }
}
