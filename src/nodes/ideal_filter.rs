#![warn(missing_docs)]
use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::OpossumError;
use crate::lightdata::{DataEnergy, LightData};
use crate::optic_ports::OpticPorts;
use crate::optical::{LightResult, Optical};
use crate::properties::{Properties, Property, Proptype};
use crate::spectrum::Spectrum;
use std::collections::HashMap;

type Result<T> = std::result::Result<T, OpossumError>;

/// Config data for an [`IdealFilter`].
#[derive(Debug, Clone)]
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
pub struct IdealFilter {
    filter_type: FilterType,
    props: Properties,
}

fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props.set(
        "name",
        Property {
            prop: Proptype::String("group".into()),
        },
    );
    props.set(
        "inverted",
        Property {
            prop: Proptype::Bool(false),
        },
    );
    props
}

impl Default for IdealFilter {
    fn default() -> Self {
        Self {
            filter_type: FilterType::Constant(1.0),
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
    pub fn new(filter_type: FilterType) -> Result<Self> {
        if let FilterType::Constant(transmission) = filter_type {
            if !(0.0..=1.0).contains(&transmission) {
                return Err(OpossumError::Other(
                    "attenuation must be in interval [0.0; 1.0]".into(),
                ));
            }
        }
        Ok(Self {
            filter_type,
            props: create_default_props(),
        })
    }
    /// Returns the filter type of this [`IdealFilter`].
    pub fn filter_type(&self) -> FilterType {
        self.filter_type.clone()
    }
    /// Sets a constant transmission value for this [`IdealFilter`].
    ///
    /// This implicitly sets the filter type to [`FilterType::Constant`].
    /// # Errors
    ///
    /// This function will return an error if a transmission factor > 1.0 is given (This would be an amplifiying filter :-) ).
    pub fn set_transmission(&mut self, transmission: f64) -> Result<()> {
        if (0.0..=1.0).contains(&transmission) {
            self.filter_type = FilterType::Constant(transmission);
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
    pub fn set_optical_density(&mut self, density: f64) -> Result<()> {
        if density >= 0.0 {
            self.filter_type = FilterType::Constant(f64::powf(10.0, -1.0 * density));
            Ok(())
        } else {
            Err(OpossumError::Other("optical densitiy must be >=0".into()))
        }
    }
    /// Returns the transmission factor of this [`IdealFilter`] expressed as optical density for the [`FilterType::Constant`].
    ///
    /// This functions `None` if the filter type is not [`FilterType::Constant`].
    pub fn optical_density(&self) -> Option<f64> {
        match self.filter_type {
            FilterType::Constant(t) => Some(-1.0 * f64::log10(t)),
            _ => None,
        }
    }
    fn analyze_energy(&mut self, incoming_data: LightResult) -> Result<LightResult> {
        let input = incoming_data.get("front");
        if let Some(Some(input)) = input {
            match input {
                LightData::Energy(e) => {
                    let mut out_spec = e.spectrum.clone();
                    match &self.filter_type {
                        FilterType::Constant(t) => {
                            if out_spec.scale_vertical(*t).is_ok() {
                                let light_data =
                                    Some(LightData::Energy(DataEnergy { spectrum: out_spec }));
                                return Ok(HashMap::from([("rear".into(), light_data)]));
                            }
                        }
                        FilterType::Spectrum(s) => {
                            out_spec.filter(s);
                            let light_data =
                                Some(LightData::Energy(DataEnergy { spectrum: out_spec }));
                            return Ok(HashMap::from([("rear".into(), light_data)]));
                        }
                    }
                }
                _ => return Err(OpossumError::Analysis("expected energy value".into())),
            }
        }
        Err(OpossumError::Analysis("error in analysis".into()))
    }
}

impl Optical for IdealFilter {
    fn node_type(&self) -> &str {
        "ideal filter"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("front").unwrap();
        ports.add_output("rear").unwrap();
        ports
    }
    fn analyze(
        &mut self,
        incoming_data: crate::optical::LightResult,
        analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> Result<crate::optical::LightResult> {
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
    fn set_property(&mut self, name: &str, prop: Property) -> Result<()> {
        if self.props.set(name, prop).is_none() {
            Err(OpossumError::Other("property not defined".into()))
        } else {
            Ok(())
        }
    }
}

impl Dottable for IdealFilter {
    fn node_color(&self) -> &str {
        "darkgray"
    }
}
