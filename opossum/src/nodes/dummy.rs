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
    optic_node::{Alignable, OpticNode, LIDT},
    optic_ports::PortType,
};

#[derive(Debug, Clone)]
/// A fake / dummy component without any optical functionality.
///
/// Any incoming light is transparently forwarded without any modification. It is mainly used for
/// development and debugging purposes. In addition it can be used as an "optical terminal" of a
/// [`NodeGroup`](crate::nodes::NodeGroup) such as the "input hole" of a cameara box which does not really
/// represent an optically active component. However, this way a group can be positioned in a scenery.
/// In addition, a [`Dummy`] can have an [`Aperture`](crate::aperture::Aperture) defined. This way, things like
/// a mask (e.g. serrated aperture) which apodized an incoming beam can be realized.
/// 
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
}
impl Default for Dummy {
    fn default() -> Self {
        let mut d = Self {
            node_attr: NodeAttr::new("dummy"),
        };
        d.update_surfaces().unwrap();
        d
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
impl AnalysisGhostFocus for Dummy {
    fn analyze(
        &mut self,
        incoming_data: crate::light_result::LightRays,
        config: &crate::analyzers::GhostFocusConfig,
        _ray_collection: &mut Vec<crate::rays::Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<crate::light_result::LightRays> {
        AnalysisGhostFocus::analyze_single_surface_node(self, incoming_data, config)
    }
}
impl AnalysisEnergy for Dummy {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        incoming_data.get(in_port).map_or_else(
            || Ok(LightResult::default()),
            |data| Ok(LightResult::from([(out_port.into(), data.clone())])),
        )
    }
}
impl Alignable for Dummy {}

impl AnalysisRayTrace for Dummy {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(data) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        if let LightData::Geometric(rays) = data {
            let mut rays = rays.clone();
            let iso = self.effective_surface_iso(in_port)?;

            let refraction_intended = true;
            if let Some(surf) = self.get_optic_surface_mut(in_port) {
                rays.refract_on_surface(surf, None, refraction_intended)?;
                if let Some(aperture) = self.ports().aperture(&PortType::Input, in_port) {
                    rays.apodize(aperture, &iso)?;
                    rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                } else {
                    return Err(OpossumError::OpticPort("input aperture not found".into()));
                };
                if let Some(aperture) = self.ports().aperture(&PortType::Output, out_port) {
                    rays.apodize(aperture, &iso)?;
                    rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                } else {
                    return Err(OpossumError::OpticPort("output aperture not found".into()));
                };

                Ok(LightResult::from([(
                    out_port.into(),
                    LightData::Geometric(rays),
                )]))
            } else {
                Err(OpossumError::Analysis("no surface found. Aborting".into()))
            }
        } else {
            Ok(LightResult::from([(out_port.into(), data.clone())]))
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
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
    fn reset_data(&mut self) {
        self.reset_optic_surfaces();
    }
    fn set_apodization_warning(&mut self, _apodized: bool) {
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
        assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn set_aperture() {
        test_set_aperture::<Dummy>("input_1", "output_1");
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
        assert_eq!(node.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["input_1"]);
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
        input.insert("output_1".into(), input_light.clone());
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
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut dummy, input).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
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
        input.insert("output_1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut dummy, input).unwrap();
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
}
