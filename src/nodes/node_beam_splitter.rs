use crate::{
    analyzer::AnalyzerType,
    error::OpossumError,
    optic_node::{Dottable, LightResult, Optical},
    optic_ports::OpticPorts, lightdata::{LightData, LightDataEnergy},
};

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Debug)]
/// An ideal beamsplitter node with a given splitting ratio.
pub struct NodeBeamSplitter {
    ratio: f32,
}

impl NodeBeamSplitter {
    /// Creates a new [`NodeBeamSplitter`] with a given splitting ratio.
    pub fn new(ratio: f32) -> Self {
        Self { ratio }
    }

    /// Returns the splitting ratio of this [`NodeBeamSplitter`].
    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    /// Sets the splitting ratio of this [`NodeBeamSplitter`].
    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio;
    }
    pub fn analyze_energy(&mut self, incoming_data: LightResult) -> Result<LightResult> {
        let in1=incoming_data.clone().into_iter().find(|data| data.0=="input1");
        let in2=incoming_data.into_iter().find(|data| data.0=="input2");

        let mut in1_energy=0.0;
        let mut in2_energy=0.0;

        if let Some(in1)=in1 {
            if let Some(in1)=in1.1 {
                match in1 {
                    LightData::Energy(e) => in1_energy=e.energy,
                    _ => return Err(OpossumError::Analysis("expected energy value".into()))
                } 
            }
        }
        if let Some(in2)=in2 {
            if let Some(in2)=in2.1 {
                match in2 {
                    LightData::Energy(e) => in2_energy=e.energy,
                    _ => return Err(OpossumError::Analysis("expected energy value".into()))
                }
            }
        }
        let out1_energy=LightDataEnergy{energy: in1_energy*self.ratio+in2_energy*(1.0-self.ratio)};
        let out2_energy =LightDataEnergy{energy: in1_energy*(1.0-self.ratio) + in2_energy*self.ratio};
        Ok(vec![("out1_trans1_refl2".into(),Some(LightData::Energy(out1_energy))),("out2_trans2_refl1".into(),Some(LightData::Energy(out2_energy)))])
    }
}

impl Default for NodeBeamSplitter {
    /// Create a 50:50 beamsplitter.
    fn default() -> Self {
        Self { ratio: 0.5 }
    }
}
impl Optical for NodeBeamSplitter {
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

impl Dottable for NodeBeamSplitter {
    fn node_color(&self) -> &str {
        "lightpink"
    }
}
