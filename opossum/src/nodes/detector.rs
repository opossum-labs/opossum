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
    optic_node::OpticNode,
    optic_ports::{OpticPorts, PortType},
    surface::{OpticalSurface, Plane, Surface},
    utils::geom_transformation::Isometry,
};
use log::warn;
use std::fmt::Debug;

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
    surface: OpticalSurface,
}
impl Default for Detector {
    fn default() -> Self {
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "in1").unwrap();
        ports.add(&PortType::Output, "out1").unwrap();
        let mut node_attr = NodeAttr::new("detector");
        node_attr.set_ports(ports);
        Self {
            light_data: Option::default(),
            node_attr,
            surface: OpticalSurface::new(Box::new(Plane::new(&Isometry::identity()))),
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
        detector.node_attr.set_name(name);
        detector
    }
}
impl OpticNode for Detector {
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn reset_data(&mut self) {
        self.light_data = None;
        self.surface.reset_hit_map();
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
impl Analyzable for Detector {}
impl AnalysisGhostFocus for Detector {}
impl AnalysisEnergy for Detector {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let (inport, outport) = if self.inverted() {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let Some(data) = incoming_data.get(inport) else {
            return Ok(LightResult::default());
        };
        let outgoing_data = LightResult::from([(outport.to_string(), data.clone())]);
        Ok(outgoing_data)
    }
}
impl AnalysisRayTrace for Detector {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let (in_port, out_port) = if self.inverted() {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let Some(data) = incoming_data.get(in_port) else {
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
            if let Some(aperture) = self.ports().aperture(&PortType::Input, in_port) {
                let rays_apodized = rays.apodize(aperture)?;
                if rays_apodized {
                    warn!("Rays have been apodized at input aperture of {}. Results might not be accurate.", self as &mut dyn OpticNode);
                }
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            } else {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            };
            self.light_data = Some(LightData::Geometric(rays.clone()));
            if let Some(aperture) = self.ports().aperture(&PortType::Output, out_port) {
                rays.apodize(aperture)?;
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            } else {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            };
            Ok(LightResult::from([(
                out_port.into(),
                LightData::Geometric(rays),
            )]))
        } else {
            Ok(LightResult::from([(out_port.into(), data.clone())]))
        }
    }

    fn get_light_data_mut(&mut self) -> Option<&mut LightData> {
        self.light_data.as_mut()
    }
    fn set_light_data(&mut self, ld: LightData) {
        self.light_data = Some(ld);
    }
}
impl Surface for Detector {
    fn get_surface_mut(&mut self, _surf_name: &str) -> &mut OpticalSurface {
        todo!()
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        lightdata::DataEnergy, nodes::test_helper::test_helper::*, optic_ports::PortType,
        spectrum_helper::create_he_ne_spec,
    };
    #[test]
    fn default() {
        let mut node = Detector::default();
        assert_eq!(node.name(), "detector");
        assert_eq!(node.node_type(), "detector");
        assert_eq!(node.inverted(), false);
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
    fn set_aperture() {
        test_set_aperture::<Detector>("in1", "out1");
    }
    #[test]
    fn ports() {
        let node = Detector::default();
        assert_eq!(node.ports().names(&PortType::Input), vec!["in1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["out1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut node = Detector::default();
        node.set_inverted(true).unwrap();
        assert_eq!(node.ports().names(&PortType::Input), vec!["out1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["in1"]);
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
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
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
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
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
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("out1".to_string(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("in1"));
        assert_eq!(output.len(), 1);
        let output = output.get("in1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
}
