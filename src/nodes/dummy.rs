#![warn(missing_docs)]
use serde_derive::Serialize;
use std::collections::HashMap;

use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::OpossumError;
use crate::optic_ports::OpticPorts;
use crate::optical::{LightResult, Optical};
use crate::properties::{Properties, Proptype, Property};

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Debug, Serialize)]
/// A fake / dummy component without any optical functionality.
///
/// Any [`LightResult`] is directly forwarded without any modification. It is mainly used for
/// development and debugging purposes.
///
/// ## Optical Ports
///   - Inputs
///     - `front`
///   - Outputs
///     - `rear`
pub struct Dummy {
    is_inverted: bool,
    name: String,
    props: Properties
}

impl Default for Dummy {
    fn default() -> Self {
        let mut props= Properties::default();
        props.set("name", Property{prop: Proptype::String("udo".into())});
        Self {
            is_inverted: Default::default(),
            name: String::from("dummy"),
            props: props
        }
    }
}
impl Dummy {
    /// Creates a new [`Dummy`] with a given name.
    pub fn new(name: &str) -> Self {
        let mut props= Properties::default();
        props.set("name", Property{prop: Proptype::String(name.into())});
        Self {
            name: name.to_owned(),
            is_inverted: false,
            props: props
        }
    }
}
impl Optical for Dummy {
    fn set_name(&mut self, name: &str) {
        self.name = name.to_owned()
    }
    fn name(&self) -> &str {
        &self.name
    }
    /// Returns "dummy" as node type.
    fn node_type(&self) -> &str {
        "dummy"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("front").unwrap();
        ports.add_output("rear").unwrap();
        ports
    }

    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _analyzer_type: &AnalyzerType,
    ) -> Result<LightResult> {
        if !self.is_inverted {
            if let Some(data) = incoming_data.get("front") {
                Ok(HashMap::from([("rear".into(), data.clone())]))
            } else {
                Ok(HashMap::from([("rear".into(), None)]))
            }
        } else if let Some(data) = incoming_data.get("rear") {
            Ok(HashMap::from([("front".into(), data.clone())]))
        } else {
            Ok(HashMap::from([("front".into(), None)]))
        }
    }
    fn set_inverted(&mut self, inverted: bool) {
        self.is_inverted = inverted;
    }
    fn inverted(&self) -> bool {
        self.is_inverted
    }
    fn properties(&self) -> Properties {
        self.props.clone()
    }
}

impl Dottable for Dummy {}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn new() {
        let node = Dummy::new("Test");
        assert_eq!(node.name, "Test");
        assert_eq!(node.inverted(), false);
    }
    #[test]
    fn default() {
        let node = Dummy::default();
        assert_eq!(node.name, "dummy");
        assert_eq!(node.inverted(), false);
    }
    #[test]
    fn set_name() {
        let mut node = Dummy::default();
        node.set_name("Test1");
        assert_eq!(node.name, "Test1")
    }
    #[test]
    fn name() {
        let mut node = Dummy::default();
        node.set_name("Test1");
        assert_eq!(node.name(), "Test1")
    }
    #[test]
    fn set_inverted() {
        let mut node = Dummy::default();
        node.set_inverted(true);
        assert_eq!(node.is_inverted, true)
    }
    #[test]
    fn inverted() {
        let mut node = Dummy::default();
        node.set_inverted(true);
        assert_eq!(node.inverted(), true)
    }
    #[test]
    fn is_detector() {
        let node = Dummy::default();
        assert_eq!(node.is_detector(), false);
    }
    #[test]
    fn node_type() {
        let node = Dummy::default();
        assert_eq!(node.node_type(), "dummy");
    }
}
