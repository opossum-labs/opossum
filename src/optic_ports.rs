use serde_derive::{Deserialize, Serialize};

use crate::{
    aperture::Aperture,
    error::{OpmResult, OpossumError},
    properties::Proptype,
};
use std::{collections::BTreeMap, fmt::Display};

/// Structure defining the optical ports (input / output terminals) of an [`Optical`](crate::optical::Optical).
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct OpticPorts {
    inputs: BTreeMap<String, Aperture>,
    outputs: BTreeMap<String, Aperture>,
    #[serde(skip)]
    inverted: bool,
}

impl OpticPorts {
    pub fn new() -> Self {
        Self::default()
    }
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
    pub fn inputs(&self) -> &BTreeMap<String, Aperture> {
        if self.inverted {
            &self.outputs
        } else {
            &self.inputs
        }
    }
    pub fn outputs(&self) -> &BTreeMap<String, Aperture> {
        if self.inverted {
            &self.inputs
        } else {
            &self.outputs
        }
    }
    pub fn create_input(&mut self, name: &str) -> OpmResult<Vec<String>> {
        if self.inputs.insert(name.into(), Aperture::None).is_none() {
            Ok(self.input_names())
        } else {
            Err(OpossumError::OpticPort(format!(
                "input port with name {} already exists",
                name
            )))
        }
    }
    pub fn create_output(&mut self, name: &str) -> OpmResult<Vec<String>> {
        if self.outputs.insert(name.into(), Aperture::None).is_none() {
            Ok(self.output_names())
        } else {
            Err(OpossumError::OpticPort(format!(
                "output port with name {} already exists",
                name
            )))
        }
    }
    pub fn set_input_aperture(&mut self, port_name: &str, aperture: Aperture) -> OpmResult<()> {
        if self.inputs.contains_key(port_name) {
            self.inputs.insert(port_name.to_owned(), aperture);
            Ok(())
        } else {
            Err(OpossumError::OpticPort(format!(
                "input port <{}> does not exist",
                port_name
            )))
        }
    }
    pub fn set_output_aperture(&mut self, port_name: &str, aperture: Aperture) -> OpmResult<()> {
        if self.outputs.contains_key(port_name) {
            self.outputs.insert(port_name.to_owned(), aperture);
            Ok(())
        } else {
            Err(OpossumError::OpticPort(format!(
                "output port <{}> does not exist",
                port_name
            )))
        }
    }
    pub fn set_apertures(&mut self, set_ports: OpticPorts) -> OpmResult<()> {
        for set_port in set_ports.inputs {
            self.set_input_aperture(&set_port.0, set_port.1)?;
        }
        for set_port in set_ports.outputs {
            self.set_output_aperture(&set_port.0, set_port.1)?;
        }
        Ok(())
    }
    pub fn check_if_port_exists(&self, port_name: &str) -> bool {
        self.inputs.contains_key(port_name) || self.outputs.contains_key(port_name)
    }
    pub fn set_inverted(&mut self, inverted: bool) {
        self.inverted = inverted;
    }
    pub fn inverted(&self) -> bool {
        self.inverted
    }
}
impl From<OpticPorts> for Proptype {
    fn from(value: OpticPorts) -> Self {
        Proptype::OpticPorts(value)
    }
}
impl Display for OpticPorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "inputs:").unwrap();
        if !&self.inputs.is_empty() {
            for port in self.inputs() {
                writeln!(f, "  <{}> {:?}", port.0, port.1).unwrap();
            }
        } else {
            writeln!(f, "  None").unwrap();
        }
        writeln!(f, "output:").unwrap();
        if !&self.outputs.is_empty() {
            for port in self.outputs() {
                writeln!(f, "  <{}> {:?}", port.0, port.1).unwrap();
            }
        } else {
            writeln!(f, "  None").unwrap();
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
        assert_eq!(ports.create_input("Test").unwrap(), vec!["Test"]);
        assert!(ports.create_input("Test").is_err());
        assert_eq!(ports.inputs.len(), 1);
    }
    #[test]
    fn add_output_ok() {
        let mut ports = OpticPorts::new();
        assert_eq!(ports.create_output("Test").unwrap(), vec!["Test"]);
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
    fn inverted() {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        assert_eq!(ports.inverted(), true);
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
            "inputs:\n  <test1> None\n  <test2> None\noutput:\n  <test3> None\n  <test4> None\n"
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
            "inputs:\n  <test3> None\n  <test4> None\noutput:\n  <test1> None\n  <test2> None\nports are inverted\n"
                .to_owned()
        );
    }
}
