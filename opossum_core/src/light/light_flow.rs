#![warn(missing_docs)]
//! # Light Flow Modeling
//!
//! Contains abstractions and algorithms for modeling the propagation of light
//! through the optical system. It represents the data structure for the edges of the underlying
//! `OpticGraph`.
//!
//! [`LightFlow`] represents the information / data flowing from one node to another node. It contains information about
//! the respective source an target port names this edge connects as well as the actual light information (stored as
//! [`LightData`]).
use crate::{
    error::{OpmResult, OpossumError},
    light::LightData,
};
use serde::Serialize;
use uom::si::f64::Length;

/// A structure for handling the propagation of ray bundles between optical nodes.
#[derive(Debug, Clone, Serialize)]
pub struct LightFlow {
    /// name of the optic port of the source node
    src_port: String,
    /// name of the optic port of the target node
    target_port: String,
    #[serde(skip)]
    /// the data (payload) "flowing" from a source to a target node.
    data: Option<LightData>,
    #[serde(skip)]
    /// the (straight) Euclidian distance between the anchor points of source and target node
    distance: Length,
}
impl LightFlow {
    /// Create a new [`LightFlow`] instance.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
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
    /// Returns a reference to the src port of this [`LightFlow`].
    #[must_use]
    pub fn src_port(&self) -> &str {
        self.src_port.as_ref()
    }
    /// Returns a reference to the target port of this [`LightFlow`].
    #[must_use]
    pub fn target_port(&self) -> &str {
        self.target_port.as_ref()
    }
    /// Returns the data of this [`LightFlow`].
    #[must_use]
    pub const fn data(&self) -> Option<&LightData> {
        self.data.as_ref()
    }
    /// Returns a mutable reference to the data of this [`LightFlow`].
    pub const fn data_mut(&mut self) -> &mut Option<LightData> {
        &mut self.data
    }
    /// Sets the data of this [`LightFlow`].
    pub fn set_data(&mut self, data: Option<LightData>) {
        self.data = data;
    }
    /// Swaps source and target port [`LightFlow`].
    pub const fn inverse(&mut self) {
        std::mem::swap(&mut self.src_port, &mut self.target_port);
    }
    /// Returns a reference to the distance of this [`LightFlow`].
    #[must_use]
    pub const fn distance(&self) -> &Length {
        &self.distance
    }
    /// Sets the distance of this [`LightFlow`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn set_distance(&mut self, distance: Length) -> OpmResult<()> {
        if !distance.is_finite() {
            return Err(OpossumError::Other(
                "distance between nodes must be finite".into(),
            ));
        }
        self.distance = distance;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::millimeter;
    use core::f64;
    use num_traits::Zero;

    #[test]
    fn new() -> OpmResult<()> {
        assert!(LightFlow::new("test1", "test2", millimeter!(f64::NAN)).is_err());
        assert!(LightFlow::new("test1", "test2", millimeter!(f64::NEG_INFINITY)).is_err());
        assert!(LightFlow::new("test1", "test2", millimeter!(f64::INFINITY)).is_err());
        let light = LightFlow::new("test1", "test2", Length::zero())?;
        assert_eq!(light.src_port, "test1");
        assert_eq!(light.target_port, "test2");
        assert!(light.data.is_none());
        assert_eq!(light.distance, Length::zero());
        Ok(())
    }
    #[test]
    fn src_port() -> OpmResult<()> {
        let light = LightFlow::new("test1", "test2", Length::zero())?;
        assert_eq!(light.src_port(), "test1");
        Ok(())
    }
    #[test]
    fn target_port() -> OpmResult<()> {
        let light = LightFlow::new("test1", "test2", Length::zero())?;
        assert_eq!(light.target_port(), "test2");
        Ok(())
    }
    #[test]
    fn distance() -> OpmResult<()> {
        let light = LightFlow::new("test1", "test2", millimeter!(100.0))?;
        assert_eq!(light.distance(), &millimeter!(100.0));
        Ok(())
    }
    #[test]
    fn set_distance() -> OpmResult<()> {
        let mut light = LightFlow::new("test1", "test2", millimeter!(100.0))?;
        assert!(light.set_distance(millimeter!(f64::NAN)).is_err());
        assert!(light.set_distance(millimeter!(f64::INFINITY)).is_err());
        assert!(light.set_distance(millimeter!(f64::NEG_INFINITY)).is_err());
        light.set_distance(millimeter!(50.0))?;
        assert_eq!(light.distance(), &millimeter!(50.0));
        Ok(())
    }
}
