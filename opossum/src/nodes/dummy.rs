#![warn(missing_docs)]
use super::node_attr::NodeAttr;
use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
        Analyzable, RayTraceConfig,
    },
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    light_result::LightResult,
    lightdata::LightData,
    optic_node::{OpticNode, LIDT},
    optic_ports::{OpticPorts, PortType},
    surface::{OpticalSurface, Plane},
    utils::geom_transformation::Isometry,
};

#[derive(Debug, Clone)]
/// A fake / dummy component without any optical functionality.
///
/// Any incoming light is transparently forwarded without any modification. It is mainly used for
/// development and debugging purposes. In addition it can be used as an "optical terminal" of a
/// [`NodeGroup`](crate::nodes::NodeGroup) such as the "input hole" of a cameara box which does not really
/// represent an optically active component. Howver this way a group can be positioned an a scene.
/// Geometrically, a [`Dummy`] node consists of a single flat surface.
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
    surface: OpticalSurface,
}
impl Default for Dummy {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("dummy");
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "front").unwrap();
        ports.add(&PortType::Output, "rear").unwrap();
        node_attr.set_ports(ports);
        Self {
            node_attr,
            surface: OpticalSurface::new(Box::new(Plane::new(&Isometry::identity()))),
        }
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
impl LIDT for Dummy {}

impl Analyzable for Dummy {}
impl AnalysisGhostFocus for Dummy {}
impl AnalysisEnergy for Dummy {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let (inport, outport) = if self.inverted() {
            ("rear", "front")
        } else {
            ("front", "rear")
        };
        incoming_data.get(inport).map_or_else(
            || Ok(LightResult::default()),
            |data| Ok(LightResult::from([(outport.into(), data.clone())])),
        )
    }
}

impl AnalysisRayTrace for Dummy {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
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
                self.surface.set_isometry(&iso);
                rays.refract_on_surface(&mut self.surface, None)?;
            } else {
                return Err(OpossumError::Analysis(
                    "no location for surface defined. Aborting".into(),
                ));
            }
            if let Some(aperture) = self.ports().aperture(&PortType::Input, inport) {
                rays.apodize(aperture)?;
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            } else {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            };
            if let Some(aperture) = self.ports().aperture(&PortType::Output, outport) {
                rays.apodize(aperture)?;
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
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
}

impl OpticNode for Dummy {
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn reset_data(&mut self) {
        self.surface.reset_hit_map();
    }
    fn get_surface_mut(&mut self, _surf_name: &str) -> &mut OpticalSurface {
        &mut self.surface
    }
}

impl Dottable for Dummy {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        lightdata::{DataEnergy, LightData},
        nodes::test_helper::test_helper::*,
        optic_ports::PortType,
        spectrum_helper::create_he_ne_spec,
    };
    #[test]
    fn default() {
        let mut node = Dummy::default();
        assert_eq!(node.name(), "dummy");
        assert_eq!(node.node_type(), "dummy");
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
        assert_eq!(node.ports().names(&PortType::Input), vec!["front"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["rear"]);
    }
    #[test]
    fn set_aperture() {
        test_set_aperture::<Dummy>("front", "rear");
    }
    #[test]
    fn as_ref_node_mut() {
        let mut node = Dummy::default();
        assert!(node.as_refnode_mut().is_err());
    }
    #[test]
    fn report() {
        let report = Dummy::default().node_report("123");
        assert!(report.is_none());
    }
    #[test]
    fn ports_inverted() {
        let mut node = Dummy::default();
        node.set_inverted(true).unwrap();
        assert_eq!(node.ports().names(&PortType::Input), vec!["rear"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["front"]);
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
        let output = AnalysisEnergy::analyze(&mut dummy, input).unwrap();
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
        let output = AnalysisEnergy::analyze(&mut dummy, input).unwrap();
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

        let output = AnalysisEnergy::analyze(&mut dummy, input).unwrap();
        assert!(output.contains_key("front"));
        assert_eq!(output.len(), 1);
        let output = output.get("front");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
}
