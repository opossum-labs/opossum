//! Data structure for the graph edges.
//!
//! [`Light`] represents the information / data flowing from one node to another node. It contains information about
//! the respective source an target port names this edge connects as well as the actual light information (stored as
//! [`LightData`]).

use crate::{lightdata::LightData, utils::geom_transformation::Isometry};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Light {
    src_port: String,
    target_port: String,
    #[serde(skip)]
    data: Option<LightData>,
    #[serde(skip)]
    isometry: Isometry
}
impl Light {
    pub fn new(src_port: &str, target_port: &str) -> Self {
        Self {
            src_port: src_port.into(),
            target_port: target_port.into(),
            data: None,
            isometry: Isometry::identity()
        }
    }
    pub fn src_port(&self) -> &str {
        self.src_port.as_ref()
    }
    pub fn target_port(&self) -> &str {
        self.target_port.as_ref()
    }
    pub const fn data(&self) -> Option<&LightData> {
        self.data.as_ref()
    }
    pub fn set_data(&mut self, data: Option<LightData>) {
        self.data = data;
    }
    pub fn inverse(&mut self) {
        let tmp = self.src_port.clone();
        self.src_port = self.target_port.clone();
        self.target_port = tmp;
    }
    pub fn isometry(&self) -> &Isometry {
        &self.isometry
    }
    pub fn set_isometry(&mut self, isometry: Isometry) {
        self.isometry = isometry;
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
        assert_eq!(light.isometry, Isometry::identity())
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
