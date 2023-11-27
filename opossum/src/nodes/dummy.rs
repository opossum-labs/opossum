#![warn(missing_docs)]
use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::OpmResult;
use crate::optic_ports::OpticPorts;
use crate::optical::{LightResult, Optical};
use crate::properties::{Properties, Proptype};
use crate::reporter::NodeReport;
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
    props: Properties,
}

impl Default for Dummy {
    fn default() -> Self {
        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        let mut props = Properties::new("dummy", "dummy");
        props.set("apertures", ports.into()).unwrap();
        Self { props }
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
        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        let mut props = Properties::new(name, "dummy");
        props.set("apertures", ports.into()).unwrap();
        Self { props }
    }
}
impl Optical for Dummy {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (src, target) = if self.properties().inverted()? {
            ("rear", "front")
        } else {
            ("front", "rear")
        };
        let data = incoming_data.get(src).unwrap_or(&None);
        Ok(HashMap::from([(target.into(), data.clone())]))
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.props.set(name, prop)
    }
    fn report(&self) -> Option<NodeReport> {
        Some(NodeReport::new(
            self.properties().node_type().unwrap(),
            self.properties().name().unwrap(),
            self.props.clone(),
        ))
    }
}

impl Dottable for Dummy {}

#[cfg(test)]
mod test {
    use std::path::Path;

    use super::*;
    use crate::{
        aperture::Aperture,
        lightdata::{DataEnergy, LightData},
        spectrum::create_he_ne_spec,
    };
    #[test]
    fn default() {
        let node = Dummy::default();
        assert_eq!(node.properties().name().unwrap(), "dummy");
        assert_eq!(node.properties().node_type().unwrap(), "dummy");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let node = Dummy::new("Test");
        assert_eq!(node.properties().name().unwrap(), "Test");
    }
    #[test]
    fn name_property() {
        let mut node = Dummy::default();
        node.set_property("name", "Test1".into()).unwrap();
        assert_eq!(node.properties().name().unwrap(), "Test1")
    }
    #[test]
    fn node_type_readonly() {
        let mut node = Dummy::default();
        assert!(node.set_property("node_type", "other".into()).is_err());
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
        assert!(node.set_input_aperture("front", aperture.clone()).is_ok());
        assert!(node.set_input_aperture("rear", aperture.clone()).is_err());
        assert!(node.set_input_aperture("no port", aperture).is_err());
    }
    #[test]
    fn set_output_aperture() {
        let mut node = Dummy::default();
        let aperture = Aperture::default();
        assert!(node.set_output_aperture("rear", aperture.clone()).is_ok());
        assert!(node.set_output_aperture("front", aperture.clone()).is_err());
        assert!(node.set_output_aperture("no port", aperture).is_err());
    }
    #[test]
    fn export_data() {
        assert!(Dummy::default().export_data(Path::new("")).is_ok());
    }
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
    fn node_type() {
        let node = Dummy::default();
        assert_eq!(node.properties().node_type().unwrap(), "dummy");
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
