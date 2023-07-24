use std::collections::HashMap;
use uom::{num_traits::Zero, si::f64::Energy};

use crate::{
    analyzer::AnalyzerType,
    error::OpossumError,
    lightdata::{DataEnergy, LightData},
    optic_node::{Dottable, LightResult, Optical},
    optic_ports::OpticPorts,
    spectrum::Spectrum,
};

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Debug)]
/// An ideal beamsplitter node with a given splitting ratio.
pub struct BeamSplitter {
    ratio: f64,
}

impl BeamSplitter {
    /// Creates a new [`BeamSplitter`] with a given splitting ratio.
    pub fn new(ratio: f64) -> Self {
        Self { ratio }
    }

    /// Returns the splitting ratio of this [`BeamSplitter`].
    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// Sets the splitting ratio of this [`BeamSplitter`].
    pub fn set_ratio(&mut self, ratio: f64) {
        self.ratio = ratio;
    }
    fn analyze_energy(&mut self, incoming_data: LightResult) -> Result<LightResult> {
        let in1 = incoming_data.get("input1");
        let in2 = incoming_data.get("input2");

        let mut in1_spectrum: Option<Spectrum> = None;
        let mut in2_spectrum: Option<Spectrum> = None;

        if let Some(Some(in1)) = in1 {
            match in1 {
                LightData::Energy(e) => in1_spectrum = Some(e.spectrum.clone()),
                _ => return Err(OpossumError::Analysis("expected DataEnergy value".into())),
            }
        }
        if let Some(Some(in2)) = in2 {
            match in2 {
                LightData::Energy(e) => in2_spectrum = Some(e.spectrum.clone()),
                _ => return Err(OpossumError::Analysis("expected DataEnergy value".into())),
            }
        }
        // let out1_energy = Some(LightData::Energy(DataEnergy {
        //     energy: in1_energy * self.ratio + in2_energy * (1.0 - self.ratio),
        // }));
        // let out2_energy = Some(LightData::Energy(DataEnergy {
        //     energy: in1_energy * (1.0 - self.ratio) + in2_energy * self.ratio,
        // }));
        Ok(HashMap::from([
            ("out1_trans1_refl2".into(), None), //out1_energy),
            ("out2_trans2_refl1".into(), None), //out2_energy),
        ]))
    }
}

impl Default for BeamSplitter {
    /// Create a 50:50 beamsplitter.
    fn default() -> Self {
        Self { ratio: 0.5 }
    }
}
impl Optical for BeamSplitter {
    fn node_type(&self) -> &str {
        "ideal beam splitter"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("input1").unwrap();
        ports.add_input("input2").unwrap();
        ports.add_output("out1_trans1_refl2").unwrap();
        ports.add_output("out2_trans2_refl1").unwrap();
        ports
    }

    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> Result<LightResult> {
        match analyzer_type {
            AnalyzerType::Energy => self.analyze_energy(incoming_data),
        }
    }
}

impl Dottable for BeamSplitter {
    fn node_color(&self) -> &str {
        "lightpink"
    }
}
