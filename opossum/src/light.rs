//! Data structure for the graph edges.
//!
//! [`Light`] represents the information / data flowing from one node to another node. It contains information about
//! the respective source an target port names this edge connects as well as the actual light information (stored as
//! [`LightData`]).

use crate::{error::{OpmResult, OpossumError}, lightdata::LightData};
use serde::Serialize;
use uom::si::f64::Length;

#[derive(Debug, Clone, Serialize)]
pub struct Light {
    /// name of the optic port of the source node
    src_port: String,
    /// name of the optic port of the target node
    target_port: String,
    #[serde(skip)]
    /// the data "flowing" from source to target node.
    data: Option<LightData>,
    #[serde(skip)]
    /// the (straight) Euclidian distance between the anchor points of source and target node
    distance: Length,
}
impl Light {
    pub fn new(src_port: &str, target_port: &str, distance: Length) -> OpmResult<Self> {
        if !distance.is_finite() {
            return Err(OpossumError::Other("distance must be finite".into()));
        }
        Ok(Self {
            src_port: src_port.into(),
            target_port: target_port.into(),
            data: None,
            distance,
        })
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
    pub const fn distance(&self) -> &Length {
        &self.distance
    }
}

#[cfg(test)]
mod test {
    use num::Zero;
    use crate::millimeter;
    use super::*;

    #[test]
    fn new() {
        assert!(Light::new("test1","test2", millimeter!(f64::NAN)).is_err());
        assert!(Light::new("test1","test2", millimeter!(f64::NEG_INFINITY)).is_err());
        assert!(Light::new("test1","test2", millimeter!(f64::INFINITY)).is_err());
        let light = Light::new("test1", "test2", Length::zero()).unwrap();
        assert_eq!(light.src_port, "test1");
        assert_eq!(light.target_port, "test2");
        assert!(light.data.is_none());
        assert_eq!(light.distance, Length::zero())
    }
    #[test]
    fn src_port() {
        let light = Light::new("test1", "test2", Length::zero()).unwrap();
        assert_eq!(light.src_port(), "test1");
    }
    #[test]
    fn target_port() {
        let light = Light::new("test1", "test2", Length::zero()).unwrap();
        assert_eq!(light.target_port(), "test2");
    }
    #[test]
    fn distance() {
        let light = Light::new("test1", "test2", millimeter!(100.0)).unwrap();
        assert_eq!(light.distance(), &millimeter!(100.0));
    }
}
