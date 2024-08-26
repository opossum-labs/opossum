#![warn(missing_docs)]
use super::node_attr::NodeAttr;
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    reporter::NodeReport,
    surface::{OpticalSurface, Plane},
};

#[derive(Debug, Clone)]
/// A fake / dummy component without any optical functionality.
///
/// Any [`LightResult`] is directly forwarded without any modification. It is mainly used for
/// development and debugging purposes. A [`Dummy`] node geometrically consists of a single flat surface.
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
        let mut node_attr = NodeAttr::new("dummy");
        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        node_attr.set_ports(ports);
        Self { node_attr }
    }
}
impl Dummy {
    /// Creates a new [`Dummy`] with a given name.
    ///
    /// # Panics
    ///
    /// This function panics if
    ///   - the default [`Dummy`] could not be constructed.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut dummy = Self::default();
        dummy.node_attr.set_name(name);
        dummy
    }
}
impl Optical for Dummy {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (inport, outport) = if self.inverted() {
            ("rear", "front")
        } else {
            ("front", "rear")
        };
        let Some(data) = incoming_data.get(inport) else {
            return Ok(LightResult::default());
        };
        if let LightData::Geometric(rays) = data {
            let mut rays = rays.clone();
            if let Some(iso) = self.effective_iso() {
                let plane = OpticalSurface::new(Box::new(Plane::new(&iso)));
                rays.refract_on_surface(&plane, None)?;
            } else {
                return Err(OpossumError::Analysis(
                    "no location for surface defined. Aborting".into(),
                ));
            }
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
            Ok(LightResult::from([(
                outport.into(),
                LightData::Geometric(rays),
            )]))
        } else {
            Ok(LightResult::from([(outport.into(), data.clone())]))
        }
    }
    fn report(&self, uuid: &str) -> Option<NodeReport> {
        Some(NodeReport::new(
            &self.node_type(),
            &self.name(),
            uuid,
            self.node_attr.properties().clone(),
        ))
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
}

impl Dottable for Dummy {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        lightdata::{DataEnergy, LightData},
        nodes::test_helper::test_helper::*,
        spectrum_helper::create_he_ne_spec,
    };
    #[test]
    fn default() {
        let mut node = Dummy::default();
        assert_eq!(node.name(), "dummy");
        assert_eq!(node.node_type(), "dummy");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.inverted(), false);
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
        node.node_attr.set_name("Test1");
        assert_eq!(node.name(), "Test1")
    }
    #[test]
    fn inverted() {
        test_inverted::<Dummy>()
    }
    #[test]
    fn ports() {
        let node = Dummy::default();
        assert_eq!(node.ports().input_names(), vec!["front"]);
        assert_eq!(node.ports().output_names(), vec!["rear"]);
    }
    #[test]
    fn set_aperture() {
        test_set_aperture::<Dummy>("front", "rear");
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
        let report = Dummy::default().report("123");
        assert!(report.is_some());
    }
    #[test]
    fn ports_inverted() {
        let mut node = Dummy::default();
        node.set_inverted(true).unwrap();
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
        test_analyze_empty::<Dummy>()
    }
    #[test]
    fn analyze_wrong() {
        let mut dummy = Dummy::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("rear".into(), input_light.clone());
        let output = dummy.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_ok() {
        let mut dummy = Dummy::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("front".into(), input_light.clone());
        let output = dummy.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("rear"));
        assert_eq!(output.len(), 1);
        let output = output.get("rear");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }

    #[test]
    fn analyze_inverse() {
        let mut dummy = Dummy::default();
        dummy.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("rear".into(), input_light.clone());

        let output = dummy.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("front"));
        assert_eq!(output.len(), 1);
        let output = output.get("front");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
}
