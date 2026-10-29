#![warn(missing_docs)]
//! Handling of input and output ports of optical elements.
//!
//! The optical ports represent an interface of an optical element in the graph: they define how
//! nodes can be connected to each other. For example, a simple filter contains one input and one
//! output port. Each port has a unique name and nothing else; what happens to light at a surface
//! is a property of the surface (see [`SurfaceConfig`](crate::core_optics::SurfaceConfig)).
//! Furthermore, [`OpticPorts`] can be inverted (see inverted optic nodes). In this case input and
//! output ports are swapped.
//! ```rust
//! use opossum_core::prelude::*;
//! use opossum_core::core_optics::OpticPorts;
//!
//! let mut ports = OpticPorts::new();
//! ports.add(&PortType::Input, "my input").unwrap();
//! ```
use crate::{
    error::{OpmResult, OpossumError},
    geometry::{Extruded, FaceIndex, SurfaceGeometry},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt::Display};
use utoipa::ToSchema;

/// Type of an [`OpticPorts`]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Serialize, Deserialize, ToSchema)]
pub enum PortType {
    /// input port, receiving [`LightData`](crate::light::LightData)
    Input,
    /// ouput port, sending [`LightData`](crate::light::LightData)
    Output,
}

impl PortType {
    /// Returns the opposite port direction (`Input` ↔ `Output`).
    ///
    /// Used to map a node's *logical* (inverted) port direction - the view returned by
    /// [`OpticNode::ports`](crate::core_optics::OpticNode::ports) and shown in the GUI - back to its
    /// *physical* stored direction, so writes resolve against the correct raw port map on an inverted
    /// node.
    ///
    /// # Returns
    ///
    /// [`PortType::Output`] for [`PortType::Input`] and vice versa.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Input => Self::Output,
            Self::Output => Self::Input,
        }
    }
}

impl Display for PortType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input => write!(f, "input"),
            Self::Output => write!(f, "output"),
        }
    }
}

/// The ports of a node, each with the face of the node's geometry it sits on (see
/// [`OpticNode::surface_ports`](crate::core_optics::OpticNode::surface_ports)).
///
/// The node decides which face every port sits on; several ports may share a face, and a face may
/// have no port at all.
#[derive(Debug, Clone, Copy)]
pub struct SurfacePorts {
    /// The input ports light enters through: their (physical) names and the face each one sits on.
    pub inputs: &'static [(&'static str, FaceIndex)],
    /// The output ports light leaves through: their (physical) names and the face each one sits
    /// on.
    pub outputs: &'static [(&'static str, FaceIndex)],
    /// The faces whose coating is the split of the light into the transmitted and the reflected
    /// part, both of which the node follows. Such a splitting face takes only a separator coating,
    /// every other face any coating but a separator. A node may follow both parts at another face
    /// too, such as a bored mirror, whose bore splits the light and whose face is a mirror.
    pub splitting: &'static [FaceIndex],
}

impl SurfacePorts {
    /// No ports: a node without a shape of its own, such as a group.
    pub const NONE: Self = Self {
        inputs: &[],
        outputs: &[],
        splitting: &[],
    };
    /// `input_1` and `output_1`, both on the one face of a single surface.
    pub const SINGLE_SURFACE: Self = Self {
        inputs: &[("input_1", SurfaceGeometry::FACE)],
        outputs: &[("output_1", SurfaceGeometry::FACE)],
        splitting: &[],
    };
    /// `input_1` on the front face and `output_1` on the rear face of an extruded solid.
    pub const THROUGH_SOLID: Self = Self {
        inputs: &[("input_1", Extruded::FRONT)],
        outputs: &[("output_1", Extruded::REAR)],
        splitting: &[],
    };
    /// Return the face the port of the given (physical) name sits on.
    ///
    /// # Returns
    ///
    /// The face, or `None` if there is no such port.
    #[must_use]
    pub fn face_of(&self, port: &str) -> Option<FaceIndex> {
        self.inputs
            .iter()
            .chain(self.outputs)
            .find(|(name, _)| *name == port)
            .map(|(_, face)| *face)
    }
}

/// Structure defining the optical ports (input / output terminals) of an [`OpticNode`](crate::core_optics::OpticNode).
#[derive(Default, Debug, Clone)]
pub struct OpticPorts {
    inputs: BTreeSet<String>,
    outputs: BTreeSet<String>,
    inverted: bool,
}

impl OpticPorts {
    /// Creates a new (empty) [`OpticPorts`] structure.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new input / output port with the given name.
    ///
    /// # Errors
    /// This function will return an error if the input port name already exists.
    pub fn add(&mut self, port_type: &PortType, name: &str) -> OpmResult<()> {
        let table = match port_type {
            PortType::Input => &mut self.inputs,
            PortType::Output => &mut self.outputs,
        };
        if table.insert(name.into()) {
            Ok(())
        } else {
            Err(OpossumError::OpticPort(format!(
                "port with name {name} already exists",
            )))
        }
    }

    /// Returns the names of the input / output ports of this [`OpticPorts`].
    ///
    /// **Note**: If the ports are marked as `inverted`, the input and output ports are swapped in the returned reference.
    /// This means that if you request input ports while `inverted` is `true`, you will actually get a reference to the output ports,
    /// and vice versa. This allows for flexible handling of port configurations in cases where the optical element's
    /// behavior is reversed. If you need access to the orirginal input or output ports regardless of the `inverted` state
    /// use the function the [`ports_raw`](OpticPorts::ports_raw) method.
    #[must_use]
    pub const fn ports(&self, port_type: &PortType) -> &BTreeSet<String> {
        let (mut input_ports, mut output_ports) = (&self.inputs, &self.outputs);
        if self.inverted {
            (input_ports, output_ports) = (output_ports, input_ports);
        }
        match port_type {
            PortType::Input => input_ports,
            PortType::Output => output_ports,
        }
    }
    /// Returns the names of the input / output ports of this [`OpticPorts`] without considering the `inverted` state.
    ///
    /// This function is necessary if you need access to the "physical" input or output ports in contrast to the "logical" ports returned
    /// by the [`ports`](OpticPorts::ports) method, which considers the `inverted` state.
    #[must_use]
    pub const fn ports_raw(&self, port_type: &PortType) -> &BTreeSet<String> {
        match port_type {
            PortType::Input => &self.inputs,
            PortType::Output => &self.outputs,
        }
    }

    /// Returns the input / output port names of this [`OpticPorts`].
    #[must_use]
    pub fn names(&self, port_type: &PortType) -> Vec<String> {
        self.ports(port_type).iter().cloned().collect()
    }

    /// Mark the [`OpticPorts`] as `inverted`.
    pub const fn set_inverted(&mut self, inverted: bool) {
        self.inverted = inverted;
    }
}
impl Display for OpticPorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "inputs:")?;
        if self.inputs.is_empty() {
            writeln!(f, "  None")?;
        } else {
            for port_name in self.ports(&PortType::Input) {
                writeln!(f, "  <{port_name}>")?;
            }
        }
        writeln!(f, "output:")?;
        if self.outputs.is_empty() {
            writeln!(f, "  None")?;
        } else {
            for port_name in self.ports(&PortType::Output) {
                writeln!(f, "  <{port_name}>")?;
            }
        }
        if self.inverted {
            writeln!(f, "ports are inverted")?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn new() {
        let ports = OpticPorts::new();
        assert_eq!(ports.inputs.len(), 0);
        assert_eq!(ports.outputs.len(), 0);
        assert!(!ports.inverted);
    }
    #[test]
    fn add_input_ok() {
        let mut ports = OpticPorts::new();
        assert!(ports.add(&PortType::Input, "Test").is_ok());
        assert_eq!(ports.inputs.len(), 1);
    }
    #[test]
    fn add_input_twice() {
        let mut ports = OpticPorts::new();
        assert!(ports.add(&PortType::Input, "Test").is_ok());
        assert!(ports.add(&PortType::Input, "Test").is_err());
        assert_eq!(ports.inputs.len(), 1);
    }
    #[test]
    fn add_output_ok() {
        let mut ports = OpticPorts::new();
        assert!(ports.add(&PortType::Output, "Test").is_ok());
        assert_eq!(ports.outputs.len(), 1);
    }
    #[test]
    fn add_output_twice() {
        let mut ports = OpticPorts::new();
        assert!(ports.add(&PortType::Output, "Test").is_ok());
        assert!(ports.add(&PortType::Output, "Test").is_err());
        assert_eq!(ports.outputs.len(), 1);
    }
    #[test]
    fn inputs() -> OpmResult<()> {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "Test1")?;
        ports.add(&PortType::Input, "Test2")?;
        ports.add(&PortType::Output, "Test3")?;
        ports.add(&PortType::Output, "Test4")?;
        let mut v = ports.names(&PortType::Input);
        v.sort();
        assert_eq!(v, vec!["Test1".to_string(), "Test2".to_string()]);
        Ok(())
    }
    #[test]
    fn inputs_inverted() -> OpmResult<()> {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        ports.add(&PortType::Input, "Test1")?;
        ports.add(&PortType::Input, "Test2")?;
        ports.add(&PortType::Output, "Test3")?;
        ports.add(&PortType::Output, "Test4")?;
        let mut v = ports.names(&PortType::Input);
        v.sort();
        assert_eq!(v, vec!["Test3".to_string(), "Test4".to_string()]);
        Ok(())
    }
    #[test]
    fn outputs() -> OpmResult<()> {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "Test1")?;
        ports.add(&PortType::Input, "Test2")?;
        ports.add(&PortType::Output, "Test3")?;
        ports.add(&PortType::Output, "Test4")?;
        let mut v = ports.names(&PortType::Output);
        v.sort();
        assert_eq!(v, vec!["Test3".to_string(), "Test4".to_string()]);
        Ok(())
    }
    #[test]
    fn outputs_inverted() -> OpmResult<()> {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        ports.add(&PortType::Input, "Test1")?;
        ports.add(&PortType::Input, "Test2")?;
        ports.add(&PortType::Output, "Test3")?;
        ports.add(&PortType::Output, "Test4")?;
        let mut v = ports.names(&PortType::Output);
        v.sort();
        assert_eq!(v, vec!["Test1".to_string(), "Test2".to_string()]);
        Ok(())
    }
    #[test]
    fn set_inverted() {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        assert!(ports.inverted);
    }
    #[test]
    fn display_empty() {
        let ports = OpticPorts::new();
        assert_eq!(
            ports.to_string(),
            "inputs:\n  None\noutput:\n  None\n".to_owned()
        );
    }
    #[test]
    fn display_entries() -> OpmResult<()> {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "test1")?;
        ports.add(&PortType::Output, "test2")?;
        assert_eq!(
            ports.to_string(),
            "inputs:\n  <test1>\noutput:\n  <test2>\n".to_owned()
        );
        Ok(())
    }
    #[test]
    fn display_entries_inverted() -> OpmResult<()> {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "test1")?;
        ports.add(&PortType::Output, "test2")?;
        ports.set_inverted(true);
        assert_eq!(
            ports.to_string(),
            "inputs:\n  <test2>\noutput:\n  <test1>\nports are inverted\n".to_owned()
        );
        Ok(())
    }
}
