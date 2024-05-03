#![warn(missing_docs)]
use log::warn;

use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::Proptype,
    refractive_index::refr_index_vaccuum,
    surface::Plane,
};
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
        let Some(data) = incoming_data.get(inport) else {
            return Ok(LightResult::default());
        };
        if let LightData::Geometric(rays) = data {
            let mut rays = rays.clone();
            if let Some(iso) = self.node_attr.isometry() {
                let plane = Plane::new(iso);
                rays.refract_on_surface(&plane, &refr_index_vaccuum())?;
            } else {
                return Err(OpossumError::Analysis(
                    "no location for surface defined. Aborting".into(),
                ));
            }
            if let Some(aperture) = self.ports().input_aperture("in1") {
                let rays_apodized = rays.apodize(aperture)?;
                if rays_apodized {
                    warn!("Rays have been apodized at input aperture of {} <{}>. Results might not be accurate.", self.node_attr.name(), self.node_attr.node_type());
                }
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
            Ok(LightResult::from([(
                outport.into(),
                LightData::Geometric(rays),
            )]))
        } else {
            Ok(LightResult::from([(outport.into(), data.clone())]))
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
    fn set_isometry(&mut self, isometry: crate::utils::geom_transformation::Isometry) {
        self.node_attr.set_isometry(isometry);
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
        analyzer::AnalyzerType, lightdata::DataEnergy, nodes::test_helper::test_helper::*,
        spectrum_helper::create_he_ne_spec,
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
        test_inverted::<Detector>()
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
    fn analyze_empty() {
        test_analyze_empty::<Detector>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = Detector::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("wrong".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_ok() {
        let mut node = Detector::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("in1".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1").unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_apodization_warning() {
        test_analyze_apodization_warning::<Detector>()
    }
    #[test]
    fn analyze_inverse() {
        let mut node = Detector::default();
        node.set_property("inverted", true.into()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("out1".to_string(), input_light.clone());

        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("in1"));
        assert_eq!(output.len(), 1);
        let output = output.get("in1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
}
