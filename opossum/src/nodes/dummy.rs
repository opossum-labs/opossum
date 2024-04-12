#![warn(missing_docs)]
use super::node_attr::NodeAttr;
use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::lightdata::LightData;
use crate::optic_ports::OpticPorts;
use crate::optical::{LightResult, Optical};
use crate::properties::Proptype;
use crate::refractive_index::refr_index_vaccuum;
use crate::reporter::NodeReport;
use crate::surface::Plane;
use std::collections::HashMap;

#[derive(Debug, Clone)]
/// A fake / dummy component without any optical functionality.
///
/// Any [`LightResult`] is directly forwarded without any modification. It is mainly used for
/// development and debugging purposes.
///
/// ## Optical Ports
///   - Inputs
///     - `front`
///   - Outputs
///     - `rear`
///
/// ## Properties
///   - `name`
///   - `inverted`
pub struct Dummy {
    node_attr: NodeAttr,
}

impl Default for Dummy {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("dummy", "dummy");
        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        node_attr.set_property("apertures", ports.into()).unwrap();
        Self { node_attr }
    }
}
impl Dummy {
    /// Creates a new [`Dummy`] with a given name.
    /// # Attributes
    /// * `name`: name of the  [`Dummy`] node
    ///
    /// # Panics
    /// This function panics if
    /// - the input port name already exists. (Theoretically impossible at this point, as the [`OpticPorts`] are created just before in this function)
    /// - the output port name already exists. (Theoretically impossible at this point, as the [`OpticPorts`] are created just before in this function)
    /// - the property `apertures` can not be set.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut dummy = Self::default();
        dummy.node_attr.set_property("name", name.into()).unwrap();
        dummy
    }
}
impl Optical for Dummy {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (inport, outport) = if self.properties().inverted()? {
            ("rear", "front")
        } else {
            ("front", "rear")
        };
        let data = incoming_data.get(inport).unwrap_or(&None);
        if let Some(LightData::Geometric(rays)) = data {
            let mut rays = rays.clone();
            let z_position = rays.absolute_z_of_last_surface() + rays.dist_to_next_surface();
            let plane = Plane::new_along_z(z_position)?;
            rays.refract_on_surface(&plane, &refr_index_vaccuum())?;
            if let Some(aperture) = self.ports().input_aperture("front") {
                rays.apodize(aperture)?;
                if let AnalyzerType::RayTrace(config) = analyzer_type {
                    rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                }
            } else {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            };
            if let Some(aperture) = self.ports().output_aperture("rear") {
                rays.apodize(aperture)?;
                if let AnalyzerType::RayTrace(config) = analyzer_type {
                    rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                }
            } else {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            };
            Ok(HashMap::from([(
                outport.into(),
                Some(LightData::Geometric(rays)),
            )]))
        } else {
            Ok(HashMap::from([(outport.into(), data.clone())]))
        }
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.node_attr.set_property(name, prop)
    }
    fn report(&self) -> Option<NodeReport> {
        Some(NodeReport::new(
            &self.node_type(),
            &self.name(),
            self.node_attr.properties().clone(),
        ))
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
}

impl Dottable for Dummy {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        aperture::Aperture,
        lightdata::{DataEnergy, LightData},
        spectrum_helper::create_he_ne_spec,
    };
    #[test]
    fn default() {
        let node = Dummy::default();
        assert_eq!(node.name(), "dummy");
        assert_eq!(node.node_type(), "dummy");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let node = Dummy::new("Test");
        assert_eq!(node.name(), "Test");
    }
    #[test]
    fn name_property() {
        let mut node = Dummy::default();
        node.set_property("name", "Test1".into()).unwrap();
        assert_eq!(node.name(), "Test1")
    }
    #[test]
    fn inverted() {
        let mut node = Dummy::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.properties().inverted().unwrap(), true)
    }
    #[test]
    fn ports() {
        let node = Dummy::default();
        assert_eq!(node.ports().input_names(), vec!["front"]);
        assert_eq!(node.ports().output_names(), vec!["rear"]);
    }
    #[test]
    fn set_input_aperture() {
        let mut node = Dummy::default();
        let aperture = Aperture::default();
        assert!(node.set_input_aperture("front", &aperture).is_ok());
        assert!(node.set_input_aperture("rear", &aperture).is_err());
        assert!(node.set_input_aperture("no port", &aperture).is_err());
    }
    #[test]
    fn set_output_aperture() {
        let mut node = Dummy::default();
        let aperture = Aperture::default();
        assert!(node.set_output_aperture("rear", &aperture).is_ok());
        assert!(node.set_output_aperture("front", &aperture).is_err());
        assert!(node.set_output_aperture("no port", &aperture).is_err());
    }
    // #[test]
    // #[ignore]
    // fn export_data() {
    //     assert!(Dummy::default().export_data(Path::new("")).is_ok());
    // }
    #[test]
    fn as_ref_node_mut() {
        let mut node = Dummy::default();
        assert!(node.as_refnode_mut().is_err());
    }
    #[test]
    fn report() {
        let report = Dummy::default().report();
        assert!(report.is_some());
    }
    #[test]
    fn ports_inverted() {
        let mut node = Dummy::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.ports().input_names(), vec!["rear"]);
        assert_eq!(node.ports().output_names(), vec!["front"]);
    }
    #[test]
    fn is_detector() {
        let node = Dummy::default();
        assert_eq!(node.is_detector(), false);
    }
    #[test]
    fn analyze_empty() {
        let mut dummy = Dummy::default();
        let mut input = LightResult::default();
        input.insert("front".into(), None);
        let output = dummy.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("rear"));
        assert_eq!(output.len(), 1);
        let output = output.get("rear").unwrap();
        assert!(output.is_none());
    }
    #[test]
    fn analyze_ok() {
        let mut dummy = Dummy::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("front".into(), Some(input_light.clone()));
        let output = dummy.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("rear"));
        assert_eq!(output.len(), 1);
        let output = output.get("rear").unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
    #[test]
    fn analyze_wrong() {
        let mut dummy = Dummy::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("rear".into(), Some(input_light.clone()));
        let output = dummy.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        let output = output.get("rear").unwrap();
        assert!(output.is_none());
    }
    #[test]
    fn analyze_inverse() {
        let mut dummy = Dummy::default();
        dummy.set_property("inverted", true.into()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("rear".into(), Some(input_light.clone()));

        let output = dummy.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("front"));
        assert_eq!(output.len(), 1);
        let output = output.get("front").unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
}
