#![warn(missing_docs)]
use std::collections::HashMap;

use crate::{
    analyzer::AnalyzerType,
    error::OpossumError,
    lightdata::{DataEnergy, LightData},
    optic_node::{Dottable, LightResult, Optical},
    optic_ports::OpticPorts,
    spectrum::{merge_spectra, Spectrum},
};

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Debug)]
/// An ideal beamsplitter node with a given splitting ratio.
///
/// ## Optical Ports
///   - Inputs
///     - `input1`
///     - `input2`
///   - Outputs
///     - `out1_trans1_refl2`
///     - `out2_trans2_refl1`
pub struct BeamSplitter {
    ratio: f64,
}

impl BeamSplitter {
    /// Creates a new [`BeamSplitter`] with a given splitting ratio.
    pub fn new(ratio: f64) -> Result<Self> {
        if (0.0..=1.0).contains(&ratio) {
            Ok(Self { ratio })
        } else {
            Err(OpossumError::Other(
                "splitting ration must be within (0.0..1.0)".into(),
            ))
        }
    }

    /// Returns the splitting ratio of this [`BeamSplitter`].
    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// Sets the splitting ratio of this [`BeamSplitter`].
    pub fn set_ratio(&mut self, ratio: f64) -> Result<()> {
        if (0.0..=1.0).contains(&ratio) {
            self.ratio = ratio;
            Ok(())
        } else {
            Err(OpossumError::Other(
                "splitting ration must be within (0.0..1.0)".into(),
            ))
        }
    }
    fn analyze_energy(&mut self, incoming_data: LightResult) -> Result<LightResult> {
        let in1 = incoming_data.get("input1");
        let in2 = incoming_data.get("input2");

        let mut out1_1_spectrum: Option<Spectrum> = None;
        let mut out1_2_spectrum: Option<Spectrum> = None;
        let mut out2_1_spectrum: Option<Spectrum> = None;
        let mut out2_2_spectrum: Option<Spectrum> = None;

        if let Some(Some(in1)) = in1 {
            match in1 {
                LightData::Energy(e) => {
                    let mut s = e.spectrum.clone();
                    s.scale_vertical(self.ratio).unwrap();
                    out1_1_spectrum = Some(s);
                    let mut s = e.spectrum.clone();
                    s.scale_vertical(1.0 - self.ratio).unwrap();
                    out1_2_spectrum = Some(s);
                }
                _ => return Err(OpossumError::Analysis("expected DataEnergy value".into())),
            }
        }
        if let Some(Some(in2)) = in2 {
            match in2 {
                LightData::Energy(e) => {
                    let mut s = e.spectrum.clone();
                    s.scale_vertical(self.ratio).unwrap();
                    out2_1_spectrum = Some(s);
                    let mut s = e.spectrum.clone();
                    s.scale_vertical(1.0 - self.ratio).unwrap();
                    out2_2_spectrum = Some(s);
                }
                _ => return Err(OpossumError::Analysis("expected DataEnergy value".into())),
            }
        }
        let out1_spec = merge_spectra(out1_1_spectrum, out2_2_spectrum);
        let out2_spec = merge_spectra(out1_2_spectrum, out2_1_spectrum);
        let mut out1_data: Option<LightData> = None;
        let mut out2_data: Option<LightData> = None;
        if let Some(out1_spec) = out1_spec {
            out1_data = Some(LightData::Energy(DataEnergy {
                spectrum: out1_spec,
            }))
        }
        if let Some(out2_spec) = out2_spec {
            out2_data = Some(LightData::Energy(DataEnergy {
                spectrum: out2_spec,
            }))
        }
        Ok(HashMap::from([
            ("out1_trans1_refl2".into(), out1_data),
            ("out2_trans2_refl1".into(), out2_data),
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
        "beam splitter"
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
            _ => Err(OpossumError::Analysis(
                "analysis type not yet implemented".into(),
            )),
        }
    }
}

impl Dottable for BeamSplitter {
    fn node_color(&self) -> &str {
        "lightpink"
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn new() {
        let splitter=BeamSplitter::new(0.5);
        assert!(splitter.is_ok());
        assert_eq!(splitter.unwrap().ratio, 0.5);
        assert!(BeamSplitter::new(-0.01).is_err());
        assert!(BeamSplitter::new(1.01).is_err());
    }
    #[test]
    fn default() {
        let splitter=BeamSplitter::default();
        assert_eq!(splitter.ratio, 0.5);
    }
    #[test]
    fn ratio() {
        let splitter=BeamSplitter::new(0.5).unwrap();
        assert_eq!(splitter.ratio(), 0.5);
    }
    #[test]
    fn set_ratio() {
        let mut splitter=BeamSplitter::new(0.0).unwrap();
        assert!(splitter.set_ratio(1.0).is_ok());
        assert_eq!(splitter.ratio, 1.0);
        assert!(splitter.set_ratio(-0.1).is_err());
        assert!(splitter.set_ratio(1.1).is_err());
    }
    #[test]
    fn node_type() {
        let splitter=BeamSplitter::new(0.0).unwrap();
        assert_eq!(splitter.node_type(), "beam splitter");
    }
    #[test]
    fn node_color() {
        let splitter=BeamSplitter::new(0.0).unwrap();
        assert_eq!(splitter.node_color(), "lightpink");
    }
}