#![warn(missing_docs)]
//! Handling of input and output ports of optical elements.
//!
//! An optical ports represents an interface of an optical element. The ports defines the way how nodes can be connected to each other.
//! For example, a simple filter contains one input and one output port. Each port has a (distinct) name and an [`Aperture`] (which is set to
//! [`Aperture::None`] by default). Furthermore, [`OpticPorts`] can be inverted (see inverted optic nodes). In this case input and output nodes
//! are swapped.
//! ```rust
//! use opossum::optic_ports::OpticPorts;
//! use nalgebra::Point2;
//! use opossum::{millimeter, aperture::{CircleConfig, Aperture}};
//! use uom::si::{f64::Length, length::millimeter};
//!
//! let mut ports=OpticPorts::new();
//! ports.create_input("my input").unwrap();
//! let circle_config = CircleConfig::new(millimeter!(1.5), millimeter!(1.0, 1.0)).unwrap();
//! ports.set_input_aperture("my input", &Aperture::BinaryCircle(circle_config)).unwrap();
//! ```
use crate::{
    aperture::Aperture, error::{OpmResult, OpossumError}, optic_port::OpticPort, properties::Proptype
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Display};

/// Structure defining the optical ports (input / output terminals) of an [`Optical`](crate::optical::Optical).
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
    /// Returns the input port names of this [`OpticPorts`].
    #[must_use]
    pub fn input_names(&self) -> Vec<String> {
        if self.inverted {
            self.outputs
                .iter()
                .map(|p| p.0.clone())
                .collect::<Vec<String>>()
        } else {
            self.inputs
                .iter()
                .map(|p| p.0.clone())
                .collect::<Vec<String>>()
        }
    }
    /// Returns the output port names of this [`OpticPorts`].
    #[must_use]
    pub fn output_names(&self) -> Vec<String> {
        if self.inverted {
            self.inputs
                .iter()
                .map(|p| p.0.clone())
                .collect::<Vec<String>>()
        } else {
            self.outputs
                .iter()
                .map(|p| p.0.clone())
                .collect::<Vec<String>>()
        }
    }
    /// Returns a reference to the input ports of this [`OpticPorts`].
    #[must_use]
    pub const fn inputs(&self) -> &BTreeMap<String, OpticPort> {
        if self.inverted {
            &self.outputs
        } else {
            &self.inputs
        }
    }
    /// Returns a reference to the output ports of this [`OpticPorts`].
    #[must_use]
    pub const fn outputs(&self) -> &BTreeMap<String, OpticPort> {
        if self.inverted {
            &self.inputs
        } else {
            &self.outputs
        }
    }
    /// Add a new input port with the given name.
    ///
    /// The port aperture is set to [`Aperture::None`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the input port name already exists.
    pub fn create_input(&mut self, name: &str) -> OpmResult<()> {
        if self.inputs.insert(name.into(), OpticPort::default()).is_none() {
            Ok(())
        } else {
            Err(OpossumError::OpticPort(format!(
                "input port with name {name} already exists",
            )))
        }
    }
    /// Add a new output port with the given name.
    ///
    /// The port aperture is set to [`Aperture::None`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the output port name already exists.
    pub fn create_output(&mut self, name: &str) -> OpmResult<()> {
        if self.outputs.insert(name.into(), OpticPort::default()).is_none() {
            Ok(())
        } else {
            Err(OpossumError::OpticPort(format!(
                "output port with name {name} already exists",
            )))
        }
    }
    /// Sets the aperture of an input port with the given name.
    ///
    /// The input port must have already been created before.
    ///
    /// # Errors
    ///
    /// This function will return an error if the port name does not exist.
    pub fn set_input_aperture(&mut self, port_name: &str, aperture: &Aperture) -> OpmResult<()> {
        if let Some(optic_port)=self.inputs.get_mut(port_name) {
            optic_port.set_aperture(aperture.clone());
            Ok(())
        } else {
            Err(OpossumError::OpticPort(format!(
                "input port <{port_name}> does not exist",
            )))
        }
    }
    /// Sets the aperture of an output port with the given name.
    ///
    /// The output port must have already been created before.
    ///
    /// # Errors
    ///
    /// This function will return an error if the port name does not exist.
    pub fn set_output_aperture(&mut self, port_name: &str, aperture: &Aperture) -> OpmResult<()> {
        if let Some(optic_port)=self.outputs.get_mut(port_name) {
            optic_port.set_aperture(aperture.clone());
            Ok(())
        } else {
            Err(OpossumError::OpticPort(format!(
                "input port <{port_name}> does not exist",
            )))
        }
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
            self.set_input_aperture(&set_port.0, &set_port.1.aperture())?;
        }
        for set_port in set_ports.outputs {
            self.set_output_aperture(&set_port.0, &set_port.1.aperture())?;
        }
        Ok(())
    }
    /// Get the [`Aperture`] of the given input port.
    ///
    /// This function returns `None` if the given port name was not found.
    #[must_use]
    pub fn input_aperture(&self, port_name: &str) -> Option<&Aperture> {
        if let Some(p) = self.inputs.get(port_name) {
            Some(p.aperture())
        } else {
            None
        }
    }
    /// Get the [`Aperture`] of the given ouput port.
    ///
    /// This function returns `None` if the given port name was not found.
    #[must_use]
    pub fn output_aperture(&self, port_name: &str) -> Option<&Aperture> {
        if let Some(p) = self.outputs.get(port_name) {
            Some(p.aperture())
        } else {
            None
        }
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
            for port in self.inputs() {
                writeln!(f, "  <{}> {:?}", port.0, port.1).unwrap();
            }
        }
        writeln!(f, "output:").unwrap();
        if self.outputs.is_empty() {
            writeln!(f, "  None").unwrap();
        } else {
            for port in self.outputs() {
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
    use crate::optic_ports::OpticPorts;
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
        assert!(ports.create_input("Test").is_ok());
        assert_eq!(ports.inputs.len(), 1);
    }
    #[test]
    fn add_input_twice() {
        let mut ports = OpticPorts::new();
        assert!(ports.create_input("Test").is_ok());
        assert!(ports.create_input("Test").is_err());
        assert_eq!(ports.inputs.len(), 1);
    }
    #[test]
    fn add_output_ok() {
        let mut ports = OpticPorts::new();
        assert!(ports.create_output("Test").is_ok());
        assert_eq!(ports.outputs.len(), 1);
    }
    #[test]
    fn add_output_twice() {
        let mut ports = OpticPorts::new();
        assert!(ports.create_output("Test").is_ok());
        assert!(ports.create_output("Test").is_err());
        assert_eq!(ports.outputs.len(), 1);
    }
    #[test]
    fn inputs() {
        let mut ports = OpticPorts::new();
        ports.create_input("Test1").unwrap();
        ports.create_input("Test2").unwrap();
        ports.create_output("Test3").unwrap();
        ports.create_output("Test4").unwrap();
        let mut v = ports.input_names();
        v.sort();
        assert_eq!(v, vec!["Test1".to_string(), "Test2".to_string()]);
    }
    #[test]
    fn inputs_inverted() {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        ports.create_input("Test1").unwrap();
        ports.create_input("Test2").unwrap();
        ports.create_output("Test3").unwrap();
        ports.create_output("Test4").unwrap();
        let mut v = ports.input_names();
        v.sort();
        assert_eq!(v, vec!["Test3".to_string(), "Test4".to_string()]);
    }
    #[test]
    fn outputs() {
        let mut ports = OpticPorts::new();
        ports.create_input("Test1").unwrap();
        ports.create_input("Test2").unwrap();
        ports.create_output("Test3").unwrap();
        ports.create_output("Test4").unwrap();
        let mut v = ports.output_names();
        v.sort();
        assert_eq!(v, vec!["Test3".to_string(), "Test4".to_string()]);
    }
    #[test]
    fn outputs_inverted() {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        ports.create_input("Test1").unwrap();
        ports.create_input("Test2").unwrap();
        ports.create_output("Test3").unwrap();
        ports.create_output("Test4").unwrap();
        let mut v = ports.output_names();
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
        ports.create_input("test1").unwrap();
        ports.create_input("test2").unwrap();
        ports.create_output("test3").unwrap();
        ports.create_output("test4").unwrap();
        assert_eq!(
            ports.to_string(),
            "inputs:\n  <test1> OpticPort { aperture: None, coating: Fresnel }\n  <test2> OpticPort { aperture: None, coating: Fresnel }\noutput:\n  <test3> OpticPort { aperture: None, coating: Fresnel }\n  <test4> OpticPort { aperture: None, coating: Fresnel }\n"
                .to_owned()
        );
    }
    #[test]
    fn display_entries_inverted() {
        let mut ports = OpticPorts::new();
        ports.create_input("test1").unwrap();
        ports.create_input("test2").unwrap();
        ports.create_output("test3").unwrap();
        ports.create_output("test4").unwrap();
        ports.set_inverted(true);
        assert_eq!(
            ports.to_string(),
            "inputs:\n  <test3> OpticPort { aperture: None, coating: Fresnel }\n  <test4> OpticPort { aperture: None, coating: Fresnel }\noutput:\n  <test1> OpticPort { aperture: None, coating: Fresnel }\n  <test2> OpticPort { aperture: None, coating: Fresnel }\nports are inverted\n"
                .to_owned()
        );
    }
}
