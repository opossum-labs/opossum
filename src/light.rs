//! Data structure for the graph edges.
//!
//! [`Light`] represents the information / data flowing from one node to another node. It contains information about
//! the respective source an target port names this edge connects as well as the actual light information (stored as
//! [`LightData`]).
use crate::lightdata::LightData;

#[derive(Debug, Clone)]
pub struct Light {
    src_port: String,
    target_port: String,
    data: Option<LightData>,
}

impl Light {
    pub fn new(src_port: &str, target_port: &str) -> Self {
        Self {
            src_port: src_port.into(),
            target_port: target_port.into(),
            data: None,
        }
    }
    pub fn src_port(&self) -> &str {
        self.src_port.as_ref()
    }
    pub fn target_port(&self) -> &str {
        self.target_port.as_ref()
    }
    pub fn data(&self) -> Option<&LightData> {
        self.data.as_ref()
    }
    pub fn set_data(&mut self, data: Option<LightData>) {
        self.data = data;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn new() {
        let light = Light::new("test1", "test2");
        assert_eq!(light.src_port, "test1");
        assert_eq!(light.target_port, "test2");
        assert!(light.data.is_none());
    }
    #[test]
    fn src_port() {
        let light = Light::new("test1", "test2");
        assert_eq!(light.src_port(), "test1");
    }
    #[test]
    fn target_port() {
        let light = Light::new("test1", "test2");
        assert_eq!(light.target_port(), "test2");
    }
}
