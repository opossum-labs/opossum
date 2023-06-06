use std::{collections::HashSet, mem::swap};
use crate::error::OpossumError;

#[derive(Default, Debug)]
pub struct OpticPorts {
    inputs: HashSet<String>,
    outputs: HashSet<String>,
}

impl OpticPorts {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn inputs(&self) -> Vec<String> {
        self.inputs.clone().into_iter().collect::<Vec<String>>()
    }
    pub fn outputs(&self) -> Vec<String> {
        self.outputs.clone().into_iter().collect::<Vec<String>>()
    }
    pub fn add_input(&mut self, name: &str) -> Result<(),OpossumError> {
        if self.inputs.insert(name.into()) {
            Ok(()) }
            else {
                Err(OpossumError::OpticPort(format!("input port with name {} already exists",name)))
            }
    }
    pub fn add_output(&mut self, name: &str) -> Result<(),OpossumError> {
        if self.outputs.insert(name.into()) {
            Ok(()) }
            else {
                Err(OpossumError::OpticPort(format!("output port with name {} already exists",name)))
            }
    }
    pub fn invert(&mut self) {
        swap(&mut self.inputs,&mut self.outputs);
    }
}

#[cfg(test)]
mod test {
    use crate::optic_ports::{OpticPorts};
    #[test]
    fn new() {
        let ports = OpticPorts::new();
        assert_eq!(ports.inputs.len(), 0);
        assert_eq!(ports.outputs.len(), 0);
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
        assert!(ports.add_input("Test").is_ok());
        assert!(ports.add_input("Test").is_err());
        assert_eq!(ports.inputs.len(), 1);
    }
    #[test]
    fn add_output_ok() {
        let mut ports = OpticPorts::new();
        assert!(ports.add_output("Test").is_ok());
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
        let mut v=ports.inputs();
        v.sort();
        assert_eq!(v, vec!["Test1".to_string(), "Test2".to_string()]);
    }
    #[test]
    fn outputs() {
        let mut ports = OpticPorts::new();
        ports.add_output("Test1").unwrap();
        ports.add_output("Test2").unwrap();
        let mut v=ports.outputs();
        v.sort();
        assert_eq!(v, vec!["Test1".to_string(), "Test2".to_string()]);
    }
}
