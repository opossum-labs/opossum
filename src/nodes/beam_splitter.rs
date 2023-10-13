#![warn(missing_docs)]
use std::collections::HashMap;

use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::{DataEnergy, LightData},
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Property, Proptype},
    spectrum::{merge_spectra, Spectrum},
};

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
///
/// ## Properties
///   - `name`
///   - `ratio`
///   - `inverted`
pub struct BeamSplitter {
    props: Properties,
}

fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props.set("name", "beam splitter".into());
    props.set("ratio", 0.5.into());
    props.set("inverted", false.into());
    props
}
impl BeamSplitter {
    /// Creates a new [`BeamSplitter`] with a given splitting ratio.
    ///
    /// ## Errors
    /// This function returns an [`OpossumError::Other`] if the splitting ratio is outside the closed interval
    /// [0.0..1.0].
    pub fn new(name: &str, ratio: f64) -> OpmResult<Self> {
        if (0.0..=1.0).contains(&ratio) {
            let mut props = create_default_props();
            props.set("ratio", ratio.into());
            props.set("name", name.into());
            Ok(Self { props })
        } else {
            Err(OpossumError::Other(
                "splitting ratio must be within (0.0..1.0)".into(),
            ))
        }
    }

    /// Returns the splitting ratio of this [`BeamSplitter`].
    pub fn ratio(&self) -> f64 {
        if let Some(value) = self.props.get("ratio") {
            if let Proptype::F64(value) = value.prop {
                return value;
            }
        }
        panic!("wrong data format")
    }

    /// Sets the splitting ratio of this [`BeamSplitter`].
    ///
    /// ## Errors
    /// This function returns an [`OpossumError::Other`] if the splitting ratio is outside the closed interval
    /// [0.0..1.0].
    pub fn set_ratio(&mut self, ratio: f64) -> OpmResult<()> {
        if (0.0..=1.0).contains(&ratio) {
            self.props.set("ratio", ratio.into());
            Ok(())
        } else {
            Err(OpossumError::Other(
                "splitting ration must be within (0.0..1.0)".into(),
            ))
        }
    }
    fn analyze_energy(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let (in1, in2) = if !self.inverted() {
            (incoming_data.get("input1"), incoming_data.get("input2"))
        } else {
            (
                incoming_data.get("out1_trans1_refl2"),
                incoming_data.get("out2_trans2_refl1"),
            )
        };
        let mut out1_1_spectrum: Option<Spectrum> = None;
        let mut out1_2_spectrum: Option<Spectrum> = None;
        let mut out2_1_spectrum: Option<Spectrum> = None;
        let mut out2_2_spectrum: Option<Spectrum> = None;

        if let Some(Some(in1)) = in1 {
            match in1 {
                LightData::Energy(e) => {
                    let mut s = e.spectrum.clone();
                    s.scale_vertical(self.ratio()).unwrap();
                    out1_1_spectrum = Some(s);
                    let mut s = e.spectrum.clone();
                    s.scale_vertical(1.0 - self.ratio()).unwrap();
                    out1_2_spectrum = Some(s);
                }
                _ => {
                    return Err(OpossumError::Analysis(
                        "expected DataEnergy value at input port".into(),
                    ))
                }
            }
        }
        if let Some(Some(in2)) = in2 {
            match in2 {
                LightData::Energy(e) => {
                    let mut s = e.spectrum.clone();
                    s.scale_vertical(self.ratio()).unwrap();
                    out2_1_spectrum = Some(s);
                    let mut s = e.spectrum.clone();
                    s.scale_vertical(1.0 - self.ratio()).unwrap();
                    out2_2_spectrum = Some(s);
                }
                _ => {
                    return Err(OpossumError::Analysis(
                        "expected DataEnergy value at input port".into(),
                    ))
                }
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
        if !self.inverted() {
            Ok(HashMap::from([
                ("out1_trans1_refl2".into(), out1_data),
                ("out2_trans2_refl1".into(), out2_data),
            ]))
        } else {
            Ok(HashMap::from([
                ("input1".into(), out1_data),
                ("input2".into(), out2_data),
            ]))
        }
    }
}

impl Default for BeamSplitter {
    /// Create a 50:50 beamsplitter.
    fn default() -> Self {
        Self {
            props: create_default_props(),
        }
    }
}
impl Optical for BeamSplitter {
    fn node_type(&self) -> &str {
        "beam splitter"
    }
    fn name(&self) -> &str {
        if let Proptype::String(name) = &self.props.get("name").unwrap().prop {
            name
        } else {
            self.node_type()
        }
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("input1").unwrap();
        ports.add_input("input2").unwrap();
        ports.add_output("out1_trans1_refl2").unwrap();
        ports.add_output("out2_trans2_refl1").unwrap();
        if self.properties().get_bool("inverted").unwrap().unwrap() {
            ports.set_inverted(true)
        }
        ports
    }

    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
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
    fn set_property(&mut self, name: &str, prop: Property) -> OpmResult<()> {
        if self.props.set(name, prop).is_none() {
            Err(OpossumError::Other("property not defined".into()))
        } else {
            Ok(())
        }
    }
    fn inverted(&self) -> bool {
        self.properties().get_bool("inverted").unwrap().unwrap()
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
    fn default() {
        let node = BeamSplitter::default();
        assert_eq!(node.ratio(), 0.5);
        assert_eq!(node.name(), "beam splitter");
        assert_eq!(node.node_type(), "beam splitter");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "lightpink");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let splitter = BeamSplitter::new("test", 0.6);
        assert!(splitter.is_ok());
        let splitter = splitter.unwrap();
        assert_eq!(splitter.name(), "test");
        assert_eq!(splitter.ratio(), 0.6);
        assert!(BeamSplitter::new("test", -0.01).is_err());
        assert!(BeamSplitter::new("test", 1.01).is_err());
    }
    #[test]
    fn ratio() {
        let splitter = BeamSplitter::new("test", 0.5).unwrap();
        assert_eq!(splitter.ratio(), 0.5);
    }
    #[test]
    fn set_ratio() {
        let mut splitter = BeamSplitter::new("test", 0.0).unwrap();
        assert!(splitter.set_ratio(1.0).is_ok());
        assert_eq!(splitter.ratio(), 1.0);
        assert!(splitter.set_ratio(-0.1).is_err());
        assert!(splitter.set_ratio(1.1).is_err());
    }
    #[test]
    fn inverted() {
        let mut node = BeamSplitter::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.inverted(), true)
    }
    #[test]
    fn ports() {
        let node = BeamSplitter::default();
        let mut input_ports=node.ports().inputs();
        input_ports.sort();
        assert_eq!(input_ports, vec!["input1", "input2"]);
        let mut output_ports= node.ports().outputs();
        output_ports.sort();
        assert_eq!(output_ports, vec!["out1_trans1_refl2", "out2_trans2_refl1"]);
    }
}
