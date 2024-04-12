#![warn(missing_docs)]
use crate::analyzer::AnalyzerType;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::LightData;
use crate::properties::Proptype;
use crate::refractive_index::refr_index_vaccuum;
use crate::surface::Plane;
use crate::{
    dottable::Dottable,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
};
use std::collections::HashMap;
use std::fmt::Debug;

use super::node_attr::NodeAttr;

/// A universal detector (so far for testing / debugging purposes).
///
/// Any [`LightData`] coming in will be stored internally for later display / export.
///
/// ## Optical Ports
///   - Inputs
///     - `in1`
///   - Outputs
///     - `out1`
///
/// ## Properties
///   - `name`
///   - `apertures`
///   - `inverted`
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different detector nodes can be "stacked" or used somewhere in between arbitrary optic nodes.
pub struct Detector {
    light_data: Option<LightData>,
    node_attr: NodeAttr,
}
impl Default for Detector {
    fn default() -> Self {
        let mut ports = OpticPorts::new();
        ports.create_input("in1").unwrap();
        ports.create_output("out1").unwrap();
        let mut node_attr = NodeAttr::new("detector", "detector");
        node_attr.set_property("apertures", ports.into()).unwrap();
        Self {
            light_data: Option::default(),
            node_attr,
        }
    }
}
impl Detector {
    /// Creates a new [`Detector`].
    /// # Attributes
    /// * `name`: name of the  [`Detector`]
    ///
    /// # Panics
    /// This function panics if
    /// - the input port name already exists. (Theoretically impossible at this point, as the [`OpticPorts`] are created just before in this function)
    /// - the output port name already exists. (Theoretically impossible at this point, as the [`OpticPorts`] are created just before in this function)
    /// - the property `apertures` can not be set.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut detector = Self::default();
        detector
            .node_attr
            .set_property("name", name.into())
            .unwrap();
        detector
    }
}
impl Optical for Detector {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (inport, outport) = if self.properties().inverted()? {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let data = incoming_data.get(inport).unwrap_or(&None);
        if let Some(LightData::Geometric(rays)) = data {
            let mut rays = rays.clone();
            let z_position = rays.absolute_z_of_last_surface() + rays.dist_to_next_surface();
            let plane = Plane::new_along_z(z_position)?;
            rays.refract_on_surface(&plane, &refr_index_vaccuum())?;
            if let Some(aperture) = self.ports().input_aperture("in1") {
                rays.apodize(aperture)?;
                if let AnalyzerType::RayTrace(config) = analyzer_type {
                    rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                }
            } else {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            };
            if let Some(aperture) = self.ports().output_aperture("out1") {
                rays.apodize(aperture)?;
                if let AnalyzerType::RayTrace(config) = analyzer_type {
                    rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                }
            } else {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            };
            self.light_data = Some(LightData::Geometric(rays.clone()));
            Ok(HashMap::from([(
                outport.into(),
                Some(LightData::Geometric(rays)),
            )]))
        } else {
            Ok(HashMap::from([(outport.into(), data.clone())]))
        }
    }
    fn is_detector(&self) -> bool {
        true
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.node_attr.set_property(name, prop)
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
}

impl Debug for Detector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.light_data {
            Some(data) => write!(f, "{data}"),
            None => write!(f, "no data"),
        }
    }
}
impl Dottable for Detector {
    fn node_color(&self) -> &str {
        "lemonchiffon"
    }
}
#[cfg(test)]
mod test {
    use crate::{
        analyzer::AnalyzerType, lightdata::DataEnergy, spectrum_helper::create_he_ne_spec,
    };

    use super::*;
    #[test]
    fn default() {
        let node = Detector::default();
        assert_eq!(node.name(), "detector");
        assert_eq!(node.node_type(), "detector");
        assert_eq!(node.is_detector(), true);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "lemonchiffon");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let node = Detector::new("test");
        assert_eq!(node.name(), "test");
    }
    #[test]
    fn inverted() {
        let mut node = Detector::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.properties().inverted().unwrap(), true)
    }
    #[test]
    fn ports() {
        let node = Detector::default();
        assert_eq!(node.ports().input_names(), vec!["in1"]);
        assert_eq!(node.ports().output_names(), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut node = Detector::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.ports().input_names(), vec!["out1"]);
        assert_eq!(node.ports().output_names(), vec!["in1"]);
    }
    #[test]
    fn analyze_ok() {
        let mut node = Detector::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("in1".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1").unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
    #[test]
    fn analyze_wrong() {
        let mut node = Detector::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("wrong".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        let output = output.get("out1").unwrap();
        assert!(output.is_none());
    }
    #[test]
    fn analyze_inverse() {
        let mut node = Detector::default();
        node.set_property("inverted", true.into()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("out1".to_string(), Some(input_light.clone()));

        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("in1"));
        assert_eq!(output.len(), 1);
        let output = output.get("in1").unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
}
