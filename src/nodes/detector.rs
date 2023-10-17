#![warn(missing_docs)]
use crate::error::OpmResult;
use crate::lightdata::LightData;
use crate::properties::{Properties, Proptype};
use crate::{
    dottable::Dottable,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
};
use std::collections::HashMap;
use std::fmt::Debug;

/// This node represents an universal detector (so far for test / debugging purposes).
///
/// Any [`LightData`] coming in will be stored internally for later display / export.
///
/// ## Optical Ports
///   - Inputs
///     - `in1`
///   - Outputs
///     - `out1`
///
/// ## Properties
///   - `name`
///   - `inverted`
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere in between arbitrary optic nodes.
pub struct Detector {
    light_data: Option<LightData>,
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props
        .create("name", "name of the detector", "detector".into())
        .unwrap();
    props
        .create("inverted", "inverse propagation?", false.into())
        .unwrap();
    props
}
impl Default for Detector {
    fn default() -> Self {
        Self {
            light_data: Default::default(),
            props: create_default_props(),
        }
    }
}
impl Detector {
    /// Creates a new [`Detector`].
    pub fn new(name: &str) -> Self {
        let mut props = create_default_props();
        props.set("name", name.into()).unwrap();
        Self {
            props,
            ..Default::default()
        }
    }
}
impl Optical for Detector {
    fn name(&self) -> &str {
        if let Proptype::String(name) = self.props.get("name").unwrap() {
            return name;
        }
        panic!("wrong format");
    }
    fn inverted(&self) -> bool {
        self.properties().get_bool("inverted").unwrap().unwrap()
    }
    fn node_type(&self) -> &str {
        "detector"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        if self.inverted() {
            ports.set_inverted(true);
        }
        ports.add_input("in1").unwrap();
        ports.add_output("out1").unwrap();
        ports
    }
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> OpmResult<LightResult> {
        if !self.inverted() {
            let data = incoming_data.get("in1").unwrap_or(&None);
            Ok(HashMap::from([("out1".into(), data.clone())]))
        } else {
            let data = incoming_data.get("out1").unwrap_or(&None);
            Ok(HashMap::from([("in1".into(), data.clone())]))
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
}

impl Debug for Detector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.light_data {
            Some(data) => write!(f, "{}", data),
            None => write!(f, "no data"),
        }
    }
}
impl Dottable for Detector {
    fn node_color(&self) -> &str {
        "lemonchiffon"
    }
}
#[cfg(test)]
mod test {
    use crate::{analyzer::AnalyzerType, lightdata::DataEnergy, spectrum::create_he_ne_spectrum};

    use super::*;
    #[test]
    fn default() {
        let node = Detector::default();
        assert_eq!(node.name(), "detector");
        assert_eq!(node.node_type(), "detector");
        assert_eq!(node.is_detector(), true);
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "lemonchiffon");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let node = Detector::new("test");
        assert_eq!(node.name(), "test");
    }
    #[test]
    fn inverted() {
        let mut node = Detector::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.inverted(), true)
    }
    #[test]
    fn ports() {
        let node = Detector::default();
        assert_eq!(node.ports().inputs(), vec!["in1"]);
        assert_eq!(node.ports().outputs(), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut node = Detector::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.ports().inputs(), vec!["out1"]);
        assert_eq!(node.ports().outputs(), vec!["in1"]);
    }
    #[test]
    fn analyze_ok() {
        let mut node = Detector::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("in1".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("out1".into()));
        assert_eq!(output.len(), 1);
        let output = output.get("out1".into()).unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
    #[test]
    fn analyze_wrong() {
        let mut node = Detector::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("wrong".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        let output = output.get("out1".into()).unwrap();
        assert!(output.is_none());
    }
    #[test]
    fn analyze_inverse() {
        let mut node = Detector::default();
        node.set_property("inverted", true.into()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("out1".into(), Some(input_light.clone()));

        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("in1".into()));
        assert_eq!(output.len(), 1);
        let output = output.get("in1".into()).unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
}
