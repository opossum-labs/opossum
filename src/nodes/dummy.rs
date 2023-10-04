#![warn(missing_docs)]
use serde_json::json;

use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::OpossumError;
use crate::optic_ports::OpticPorts;
use crate::optical::{LightResult, Optical};
use crate::properties::{Properties, Property, Proptype};
use std::collections::HashMap;

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Debug)]
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
    props: Properties,
}

fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props.set(
        "name",
        Property {
            prop: Proptype::String("dummy".into()),
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

impl Default for Dummy {
    fn default() -> Self {
        Self {
            props: create_default_props(),
        }
    }
}
impl Dummy {
    /// Creates a new [`Dummy`] with a given name.
    pub fn new(name: &str) -> Self {
        let mut props = create_default_props();
        props.set(
            "name",
            Property {
                prop: Proptype::String(name.into()),
            },
        );
        Self { props }
    }
}
impl Optical for Dummy {
    fn set_name(&mut self, name: &str) {
        self.props.set(
            "name",
            Property {
                prop: Proptype::String(name.into()),
            },
        );
    }
    fn name(&self) -> &str {
        if let Some(value) = self.props.get("name") {
            if let Proptype::String(name) = &value.prop {
                return name;
            }
        }
        panic!("wonrg format");
    }
    /// Returns "dummy" as node type.
    fn node_type(&self) -> &str {
        "dummy"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("front").unwrap();
        ports.add_output("rear").unwrap();
        if self.properties().get_bool("inverted").unwrap().unwrap() {
            ports.set_inverted(true)
        }
        ports
    }

    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _analyzer_type: &AnalyzerType,
    ) -> Result<LightResult> {
        if !self.inverted() {
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
    fn inverted(&self) -> bool {
        self.properties().get_bool("inverted").unwrap().unwrap()
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
    fn report(&self) -> serde_json::Value {
        json!({"type": self.node_type(),
        "name": self.name()})
    }
}

impl Dottable for Dummy {}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn new() {
        let node = Dummy::new("Test");
        assert_eq!(node.name(), "Test");
        assert_eq!(node.inverted(), false);
    }
    #[test]
    fn default() {
        let node = Dummy::default();
        assert_eq!(node.name(), "dummy");
        assert_eq!(node.inverted(), false);
    }
    #[test]
    fn name() {
        let mut node = Dummy::default();
        node.set_name("Test1");
        assert_eq!(node.name(), "Test1")
    }
    #[test]
    fn inverted() {
        let mut node = Dummy::default();
        node.set_property("inverted", true.into()).unwrap();
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
