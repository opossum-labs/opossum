use serde_derive::Serialize;

use crate::error::OpossumError;
use std::{collections::HashSet, fmt::Display};

/// Structure defining the optical ports (input / output terminals) of an [`Optical`](crate::optical::Optical).
#[derive(Default, Debug, Clone, Serialize)]
pub struct OpticPorts {
    inputs: HashSet<String>,
    outputs: HashSet<String>,
    inverted: bool,
}

impl OpticPorts {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn inputs(&self) -> Vec<String> {
        if self.inverted {
            self.outputs.clone().into_iter().collect::<Vec<String>>()
        } else {
            self.inputs.clone().into_iter().collect::<Vec<String>>()
        }
    }
    pub fn outputs(&self) -> Vec<String> {
        if self.inverted {
            self.inputs.clone().into_iter().collect::<Vec<String>>()
        } else {
            self.outputs.clone().into_iter().collect::<Vec<String>>()
        }
    }
    pub fn add_input(&mut self, name: &str) -> Result<Vec<String>, OpossumError> {
        if self.inputs.insert(name.into()) {
            Ok(self.inputs())
        } else {
            Err(OpossumError::OpticPort(format!(
                "input port with name {} already exists",
                name
            )))
        }
    }
    pub fn add_output(&mut self, name: &str) -> Result<Vec<String>, OpossumError> {
        if self.outputs.insert(name.into()) {
            Ok(self.outputs())
        } else {
            Err(OpossumError::OpticPort(format!(
                "output port with name {} already exists",
                name
            )))
        }
    }

    pub fn check_if_port_exists(&self, port_name: &str) -> bool{
        if self.inputs.contains(port_name) {
            true
        } else if self.outputs.contains(port_name) {
            true
        }
        else{
            false
        }
    }

    // pub fn get_port(&self, port_name: &str, input_flag: bool)-> Result<String, OpossumError>{
    //     if input_flag & self.inputs.contains(port_name){
    //         Ok(self.inputs.get(port_name).unwrap().to_owned())
    //     }
    //     else if !input_flag & self.outputs.contains(port_name){
    //         Ok(self.outputs.get(port_name).unwrap().to_owned())
    //     }
    //     else{
    //         Err(OpossumError::OpticPort(format!(
    //             "a port with name {} does not exist",
    //             port_name
    //         )))
    //     }
    // }

    // pub fn set_port(&mut self, target_port: &str, src_node: &OpticNode, src_port: &str, input_flag: bool) -> Result<Vec<String>, OpossumError>{
    //     let port = src_node.name().to_owned() + src_port;
        
    //     if input_flag {
    //         self.inputs.remove(target_port);
    //         self.add_input(&port)?;
    //         Ok(self.outputs())
    //     } else {
    //         self.outputs.remove(target_port);
    //         self.add_output(&port)?;
    //         Ok(self.outputs())
    //     }
    // }

    pub fn set_inverted(&mut self, inverted: bool) {
        self.inverted = inverted;
    }

    pub fn inverted(&self) -> bool {
        self.inverted
    }
}

impl Display for OpticPorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "inputs:").unwrap();
        if !&self.inputs.is_empty() {
            let mut ports = self.inputs();
            ports.sort();
            for port in ports {
                writeln!(f, "  <{}>", port).unwrap();
            }
        } else {
            writeln!(f, "  None").unwrap();
        }
        writeln!(f, "output:").unwrap();
        if !&self.outputs.is_empty() {
            let mut ports = self.outputs();
            ports.sort();
            for port in ports {
                writeln!(f, "  <{}>", port).unwrap();
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
        assert!(ports.add_input("Test").is_ok());
        assert_eq!(ports.inputs.len(), 1);
    }
    #[test]
    fn add_input_twice() {
        let mut ports = OpticPorts::new();
        assert_eq!(ports.add_input("Test").unwrap(), vec!["Test"]);
        assert!(ports.add_input("Test").is_err());
        assert_eq!(ports.inputs.len(), 1);
    }
    #[test]
    fn add_output_ok() {
        let mut ports = OpticPorts::new();
        assert_eq!(ports.add_output("Test").unwrap(), vec!["Test"]);
        assert_eq!(ports.outputs.len(), 1);
    }
    #[test]
    fn add_output_twice() {
        let mut ports = OpticPorts::new();
        assert!(ports.add_output("Test").is_ok());
        assert!(ports.add_output("Test").is_err());
        assert_eq!(ports.outputs.len(), 1);
    }
    #[test]
    fn inputs() {
        let mut ports = OpticPorts::new();
        ports.add_input("Test1").unwrap();
        ports.add_input("Test2").unwrap();
        ports.add_output("Test3").unwrap();
        ports.add_output("Test4").unwrap();
        let mut v = ports.inputs();
        v.sort();
        assert_eq!(v, vec!["Test1".to_string(), "Test2".to_string()]);
    }
    #[test]
    fn inputs_inverted() {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        ports.add_input("Test1").unwrap();
        ports.add_input("Test2").unwrap();
        ports.add_output("Test3").unwrap();
        ports.add_output("Test4").unwrap();
        let mut v = ports.inputs();
        v.sort();
        assert_eq!(v, vec!["Test3".to_string(), "Test4".to_string()]);
    }
    #[test]
    fn outputs() {
        let mut ports = OpticPorts::new();
        ports.add_input("Test1").unwrap();
        ports.add_input("Test2").unwrap();
        ports.add_output("Test3").unwrap();
        ports.add_output("Test4").unwrap();
        let mut v = ports.outputs();
        v.sort();
        assert_eq!(v, vec!["Test3".to_string(), "Test4".to_string()]);
    }
    #[test]
    fn outputs_inverted() {
        let mut ports = OpticPorts::new();
        ports.set_inverted(true);
        ports.add_input("Test1").unwrap();
        ports.add_input("Test2").unwrap();
        ports.add_output("Test3").unwrap();
        ports.add_output("Test4").unwrap();
        let mut v = ports.outputs();
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
        ports.add_input("test1").unwrap();
        ports.add_input("test2").unwrap();
        ports.add_output("test3").unwrap();
        ports.add_output("test4").unwrap();
        assert_eq!(
            ports.to_string(),
            "inputs:\n  <test1>\n  <test2>\noutput:\n  <test3>\n  <test4>\n".to_owned()
        );
    }
    #[test]
    fn display_entries_inverted() {
        let mut ports = OpticPorts::new();
        ports.add_input("test1").unwrap();
        ports.add_input("test2").unwrap();
        ports.add_output("test3").unwrap();
        ports.add_output("test4").unwrap();
        ports.set_inverted(true);
        assert_eq!(
            ports.to_string(),
            "inputs:\n  <test3>\n  <test4>\noutput:\n  <test1>\n  <test2>\nports are inverted\n"
                .to_owned()
        );
    }
}
