use crate::analyzer::AnalyzerType;
use crate::error::OpossumError;
use crate::lightdata::{LightData, LightDataEnergy};
use crate::optic_node::{Dottable, LightResult, Optical};
use crate::optic_ports::OpticPorts;
use std::collections::HashMap;
use uom::num_traits::Zero;
use uom::si::f64::Energy;

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Debug)]
/// An ideal filter with given transmission or optical density.
pub struct IdealFilter {
    transmission: f64
}

impl IdealFilter {
    /// Creates a new [`IdealFilter`] with a given energy transmission factor.
    ///
    /// # Errors
    ///
    /// This function will return an error if a transmission factor > 1.0 is given (This would be an amplifiying filter :-) ).
    pub fn new(transmission: f64) -> Result<Self> {
        if transmission <= 1.0 {
            Ok(Self { transmission })
        } else {
            Err(OpossumError::Other("attenuation must be <= 1.0".into()))
        }
    }
    /// Returns the transmission factor of this [`IdealFilter`].
    pub fn transmission(&self) -> f64 {
        self.transmission
    }
    /// Sets the transmission of this [`IdealFilter`].
    ///
    /// # Errors
    ///
    /// This function will return an error if a transmission factor > 1.0 is given (This would be an amplifiying filter :-) ).
    pub fn set_transmission(&mut self, transmission: f64) -> Result<()> {
        if transmission <= 1.0 {
            self.transmission = transmission;
            Ok(())
        } else {
            Err(OpossumError::Other("attenuation must be <=1.0".into()))
        }
    }
    /// Sets the transmission of this [`IdealFilter`] expressed as optical density.
    ///
    /// # Errors
    ///
    /// This function will return an error if an optical density < 0.0 was given.
    pub fn set_optical_density(&mut self, density: f64) -> Result<()> {
        if density >= 0.0 {
            self.transmission = f64::powf(10.0, -1.0 * density);
            Ok(())
        } else {
            Err(OpossumError::Other("optical densitiy must be >=0".into()))
        }
    }
    /// Returns the transmission facotr of this [`IdealFilter`] expressed as optical density.
    pub fn optical_density(&self) -> f64 {
        -1.0 * f64::log10(self.transmission)
    }
    fn analyze_energy(&mut self, incoming_data: LightResult) -> Result<LightResult> {
        let input = incoming_data.get("front");

        let mut input_energy = Energy::zero();

        if let Some(Some(input)) = input {
            match input {
                LightData::Energy(e) => input_energy = e.energy,
                _ => return Err(OpossumError::Analysis("expected energy value".into())),
            }
        }
        let output_energy = Some(LightData::Energy(LightDataEnergy {
            energy: input_energy * self.transmission,
        }));
        Ok(HashMap::from([("rear".into(), output_energy)]))
    }
}

impl Optical for IdealFilter {
    /// Returns "dummy" as node type.
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
        incoming_data: crate::optic_node::LightResult,
        analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> Result<crate::optic_node::LightResult> {
        match analyzer_type {
            AnalyzerType::Energy => self.analyze_energy(incoming_data),
        }
    }
}

impl Dottable for IdealFilter {
    fn node_color(&self) -> &str {
        "darkgray"
    }
}
