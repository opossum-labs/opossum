#![warn(missing_docs)]
//! Handling of input and output ports of optical elements.
//!
//! The optical ports represent an interface of an optical element. The ports define the way how nodes can be connected to each other.
//! For example, a simple filter contains one input and one output port. Each port has a unique name, an [`Aperture`] (set to
//! [`Aperture::None`] by default), and a [`CoatingType`] ([`CoatingType::IdealAR`] by default). Furthermore, [`OpticPorts`] can be
//! inverted (see inverted optic nodes). In this case input and output ports are swapped.
//! ```rust
//! use opossum::optic_ports::{OpticPorts, PortType};
//! use nalgebra::Point2;
//! use opossum::{millimeter, aperture::{CircleConfig, Aperture}};
//! use uom::si::{f64::Length, length::millimeter};
//!
//! let mut ports = OpticPorts::new();
//! ports.add(&PortType::Input, "my input").unwrap();
//! let circle_config = CircleConfig::new(millimeter!(1.5), millimeter!(1.0, 1.0)).unwrap();
//! ports.set_aperture(&PortType::Input, "my input", &Aperture::BinaryCircle(circle_config)).unwrap();
//! ```
use crate::{
    aperture::Aperture,
    coatings::CoatingType,
    error::{OpmResult, OpossumError},
    optic_port::OpticPort,
    properties::Proptype,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Display};
/// Type of an [`OpticPort`]
pub enum PortType {
    /// input port, receiving [`LightData`](crate::lightdata::LightData)
    Input,
    /// ouput port, sending [`LightData`](crate::lightdata::LightData)
    Output,
}
/// Structure defining the optical ports (input / output terminals) of an [`OpticNode`](crate::optic_node::OpticNode).
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct OpticPorts {
    inputs: BTreeMap<String, OpticPort>,
    outputs: BTreeMap<String, OpticPort>,
    #[serde(skip)]
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
    /// The port aperture is set to the default [`Aperture::None`]. The coating is set to the default [`CoatingType::IdealAR`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the input port name already exists.
    pub fn add(&mut self, port_type: &PortType, name: &str) -> OpmResult<()> {
        let table = match port_type {
            PortType::Input => &mut self.inputs,
            PortType::Output => &mut self.outputs,
        };
        if table.insert(name.into(), OpticPort::default()).is_none() {
            Ok(())
        } else {
            Err(OpossumError::OpticPort(format!(
                "port with name {name} already exists",
            )))
        }
    }
    /// Returns a reference to the input / output ports of this [`OpticPorts`].
    #[must_use]
    pub const fn ports(&self, port_type: &PortType) -> &BTreeMap<String, OpticPort> {
        let (mut input_ports, mut output_ports) = (&self.inputs, &self.outputs);
        if self.inverted {
            (input_ports, output_ports) = (output_ports, input_ports);
        }
        match port_type {
            PortType::Input => input_ports,
            PortType::Output => output_ports,
        }
    }
    /// Returns the input / output port names of this [`OpticPorts`].
    #[must_use]
    pub fn names(&self, port_type: &PortType) -> Vec<String> {
        self.ports(port_type)
            .iter()
            .map(|p| p.0.clone())
            .collect::<Vec<String>>()
    }
    /// Sets the aperture of an port with the given name.
    ///
    /// The port must have already been created before.
    ///
    /// # Errors
    ///
    /// This function will return an error if the port name does not exist.
    pub fn set_aperture(
        &mut self,
        port_type: &PortType,
        port_name: &str,
        aperture: &Aperture,
    ) -> OpmResult<()> {
        let (mut input_ports, mut output_ports) = (&mut self.inputs, &mut self.outputs);
        if self.inverted {
            (input_ports, output_ports) = (output_ports, input_ports);
        }
        let ports: &mut BTreeMap<String, OpticPort> = match port_type {
            PortType::Input => input_ports,
            PortType::Output => output_ports,
        };
        ports.get_mut(port_name).map_or_else(
            || {
                Err(OpossumError::OpticPort(format!(
                    "port name <{port_name}> does not exist",
                )))
            },
            |optic_port| {
                optic_port.set_aperture(aperture.clone());
                Ok(())
            },
        )
    }
    /// Sets the coating of a port with the given name.
    ///
    /// The port must have already been created before.
    ///
    /// # Errors
    ///
    /// This function will return an error if the port name does not exist.
    pub fn set_coating(
        &mut self,
        port_type: &PortType,
        port_name: &str,
        coating: &CoatingType,
    ) -> OpmResult<()> {
        let (mut input_ports, mut output_ports) = (&mut self.inputs, &mut self.outputs);
        if self.inverted {
            (input_ports, output_ports) = (output_ports, input_ports);
        }
        let ports: &mut BTreeMap<String, OpticPort> = match port_type {
            PortType::Input => input_ports,
            PortType::Output => output_ports,
        };
        ports.get_mut(port_name).map_or_else(
            || {
                Err(OpossumError::OpticPort(format!(
                    "port <{port_name}> does not exist",
                )))
            },
            |optic_port| {
                optic_port.set_coating(coating.clone());
                Ok(())
            },
        )
    }
    /// Sets the (input & ouput port) apertures of this [`OpticPorts`] from another [`OpticPorts`].
    ///
    /// This is a convenience function during deserialization of an optical element.
    ///
    /// # Errors
    ///
    /// This function will return an error if the port names in `set_ports` are not found.
    pub fn set_apertures(&mut self, set_ports: Self) -> OpmResult<()> {
        for set_port in set_ports.inputs {
            self.set_aperture(&PortType::Input, &set_port.0, set_port.1.aperture())?;
        }
        for set_port in set_ports.outputs {
            self.set_aperture(&PortType::Output, &set_port.0, set_port.1.aperture())?;
        }
        Ok(())
    }
    /// Get the [`Aperture`] of the port with the given name.
    ///
    /// This function returns `None` if the given port name was not found.
    #[must_use]
    pub fn aperture(&self, port_type: &PortType, port_name: &str) -> Option<&Aperture> {
        self.ports(port_type)
            .get(port_name)
            .map(OpticPort::aperture)
    }
    /// Get the coating of the given input port.
    ///
    /// This function returns `None` if the given port name was not found.
    #[must_use]
    pub fn coating(&self, port_type: &PortType, port_name: &str) -> Option<&CoatingType> {
        self.ports(port_type).get(port_name).map(OpticPort::coating)
    }
    /// Mark the [`OpticPorts`] as `inverted`.
    ///
    /// This swaps input and output ports.
    pub fn set_inverted(&mut self, inverted: bool) {
        self.inverted = inverted;
    }
}
impl From<OpticPorts> for Proptype {
    fn from(value: OpticPorts) -> Self {
        Self::OpticPorts(value)
    }
}
impl Display for OpticPorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "inputs:").unwrap();
        if self.inputs.is_empty() {
            writeln!(f, "  None").unwrap();
        } else {
            for port in self.ports(&PortType::Input) {
                writeln!(f, "  <{}> {:?}", port.0, port.1).unwrap();
            }
        }
        writeln!(f, "output:").unwrap();
        if self.outputs.is_empty() {
            writeln!(f, "  None").unwrap();
        } else {
            for port in self.ports(&PortType::Output) {
                writeln!(f, "  <{}> {:?}", port.0, port.1).unwrap();
            }
        }
        if self.inverted {
            writeln!(f, "ports are inverted").unwrap();
        }
        Ok(())
    }
}
#[cfg(test)]
mod test {
    use crate::optic_ports::{OpticPorts, PortType};
    #[test]
    fn new() {
        let ports = OpticPorts::new();
        assert_eq!(ports.inputs.len(), 0);
        assert_eq!(ports.outputs.len(), 0);
        assert_eq!(ports.inverted, false);
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
    fn inputs() {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "Test1").unwrap();
        ports.add(&PortType::Input, "Test2").unwrap();
        ports.add(&PortType::Output, "Test3").unwrap();
        ports.add(&PortType::Output, "Test4").unwrap();
        let mut v = ports.names(&PortType::Input);
        v.sort();
        assert_eq!(v, vec!["Test1".to_string(), "Test2".to_string()]);
    }
    #[test]
    fn inputs_inverted() {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        ports.add(&PortType::Input, "Test1").unwrap();
        ports.add(&PortType::Input, "Test2").unwrap();
        ports.add(&PortType::Output, "Test3").unwrap();
        ports.add(&PortType::Output, "Test4").unwrap();
        let mut v = ports.names(&PortType::Input);
        v.sort();
        assert_eq!(v, vec!["Test3".to_string(), "Test4".to_string()]);
    }
    #[test]
    fn outputs() {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "Test1").unwrap();
        ports.add(&PortType::Input, "Test2").unwrap();
        ports.add(&PortType::Output, "Test3").unwrap();
        ports.add(&PortType::Output, "Test4").unwrap();
        let mut v = ports.names(&PortType::Output);
        v.sort();
        assert_eq!(v, vec!["Test3".to_string(), "Test4".to_string()]);
    }
    #[test]
    fn outputs_inverted() {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        ports.add(&PortType::Input, "Test1").unwrap();
        ports.add(&PortType::Input, "Test2").unwrap();
        ports.add(&PortType::Output, "Test3").unwrap();
        ports.add(&PortType::Output, "Test4").unwrap();
        let mut v = ports.names(&PortType::Output);
        v.sort();
        assert_eq!(v, vec!["Test1".to_string(), "Test2".to_string()]);
    }
    #[test]
    fn set_inverted() {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        assert_eq!(ports.inverted, true);
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
    fn display_entries() {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "test1").unwrap();
        ports.add(&PortType::Input, "test2").unwrap();
        ports.add(&PortType::Output, "test3").unwrap();
        ports.add(&PortType::Output, "test4").unwrap();
        assert_eq!(
            ports.to_string(),
            "inputs:\n  <test1> OpticPort { aperture: None, coating: IdealAR }\n  <test2> OpticPort { aperture: None, coating: IdealAR }\noutput:\n  <test3> OpticPort { aperture: None, coating: IdealAR }\n  <test4> OpticPort { aperture: None, coating: IdealAR }\n"
                .to_owned()
        );
    }
    #[test]
    fn display_entries_inverted() {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "test1").unwrap();
        ports.add(&PortType::Input, "test2").unwrap();
        ports.add(&PortType::Output, "test3").unwrap();
        ports.add(&PortType::Output, "test4").unwrap();
        ports.set_inverted(true);
        assert_eq!(
            ports.to_string(),
            "inputs:\n  <test3> OpticPort { aperture: None, coating: IdealAR }\n  <test4> OpticPort { aperture: None, coating: IdealAR }\noutput:\n  <test1> OpticPort { aperture: None, coating: IdealAR }\n  <test2> OpticPort { aperture: None, coating: IdealAR }\nports are inverted\n"
                .to_owned()
        );
    }
}
