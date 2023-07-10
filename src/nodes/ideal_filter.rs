use std::collections::HashMap;
use crate::analyzer::AnalyzerType;
use crate::error::OpossumError;
use crate::lightdata::{LightData, LightDataEnergy};
use crate::optic_node::{Dottable, Optical, LightResult};
use crate::optic_ports::OpticPorts;

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Debug)]
/// An ideal filter with given filter ratio.
pub struct IdealFilter {
    attenuation: f64,
}

impl IdealFilter {
    pub fn new(attenuation: f64) -> Result<Self> {
        if attenuation <= 1.0 {
            Ok(Self { attenuation })
        } else {
            Err(OpossumError::Other("attenuation must be <=1.0".into()))
        }
    }
    pub fn attenuation(&self) -> f64 {
        self.attenuation
    }
    pub fn set_attenuation(&mut self, attenuation: f64) -> Result<()> {
        if attenuation <= 1.0 {
            self.attenuation = attenuation;
            Ok(())
        } else {
            Err(OpossumError::Other("attenuation must be <=1.0".into()))
        }
    }
    pub fn set_optical_density(&mut self, density: f64) -> Result<()> {
        if density>0.0 {
            self.attenuation = f64::powf(10.0, -1.0 * density);
            Ok(())
        }  else {
            Err(OpossumError::Other("optical densitiy must be >0".into()))
        }
    }
    pub fn optical_density(&self) -> f64 {
        -1.0*f64::log10(self.attenuation)
    }
    pub fn analyze_energy(&mut self, incoming_data: LightResult) -> Result<LightResult> {
        let input = incoming_data.get("front");

        let mut input_energy = 0.0;
    
        if let Some(Some(input)) = input {
            match input {
                LightData::Energy(e) => input_energy = e.energy,
                _ => return Err(OpossumError::Analysis("expected energy value".into())),
            }
        }
        let output_energy = Some(LightData::Energy(LightDataEnergy {
            energy: input_energy * self.attenuation
        }));
        Ok(HashMap::from([
            ("rear".into(), output_energy),
        ]))
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
    fn analyze(&mut self, incoming_data: crate::optic_node::LightResult, analyzer_type: &crate::analyzer::AnalyzerType) -> Result<crate::optic_node::LightResult> {
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
