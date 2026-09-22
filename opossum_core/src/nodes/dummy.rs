#![warn(missing_docs)]
//! A fake / dummy optical element without active modification of incoming light.

use opm_macros_lib::OpmNode;

use crate::{
    analyzers::{
        RayTraceConfig, energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{NodeAttr, OpticNode, OpticNodeExt, optic_node_ext::single_io_port_names},
    error::OpmResult,
    light::LightResult,
    nodes::NodeRegistration,
};

inventory::submit! {
    NodeRegistration::new::<Dummy>("dummy", "dummy node")
}

#[derive(OpmNode, Debug, Clone)]
#[opm_node("gray")]
/// A fake / dummy component without any optical functionality.
///
/// Any incoming light is transparently forwarded without any modification. It is mainly used for
/// development and debugging purposes. In addition, it can be used as an "optical terminal" of a
/// [`NodeGroup`](crate::nodes::NodeGroup) such as the "input hole" of a camera box which does not really
/// represent an optically active component. However, this way a group can be positioned in a scenery.
/// Similar to all other nodes, a [`Dummy`] can have an [`Aperture`](crate::apertures::Aperture) defined.
/// This way, things like a mask (e.g. serrated aperture) which apodizes an incoming beam can be realized.
///
/// Geometrically, a [`Dummy`] node consists of a single flat surface.
///
/// ## Optical Ports
///   - Inputs
///     - `input_1`
///   - Outputs
///     - `output_1`
///
/// ## Properties
///   - `name`
///   - `inverted`
pub struct Dummy {
    node_attr: NodeAttr,
}

impl Default for Dummy {
    fn default() -> Self {
        let mut dummy = Self {
            node_attr: NodeAttr::new("dummy"),
        };
        dummy
            .update_surfaces()
            .expect("Failed to initialize dummy surfaces");
        dummy
    }
}

impl Dummy {
    /// Creates a new [`Dummy`] with a given name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut dummy = Self::default();
        dummy.node_attr.set_name(name);
        dummy
    }
}

impl AnalysisGhostFocus for Dummy {}
impl AnalysisEnergy for Dummy {}

impl AnalysisRayTrace for Dummy {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let (in_port, _) = single_io_port_names(self)?;
        self.unified_analyze_single_surface_node(incoming_data, config, &in_port, None)
    }
}

impl OpticNode for Dummy {
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::energy::EnergyConfig,
        apertures::{Aperture, ApertureType},
        core_optics::{NodeAttrExt, PortType, node_attr::NodePositioning},
        joule,
        light::{LightData, Ray, Rays, spectrum_helper::create_he_ne_spec},
        millimeter, nanometer,
        nodes::test_helper::test_helper::*,
        reporting::node_report::NodeReportResult,
        utils::geom_transformation::Isometry,
    };

    #[test]
    fn default() {
        let node = Dummy::default();
        assert_eq!(node.name(), "dummy");
        assert_eq!(node.node_type(), "dummy");
        assert!(!node.inverted());
    }

    #[test]
    fn dottable() {
        let node = Dummy::default();
        assert_eq!(node.node_color(), "gray");
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
        assert_eq!(node.name(), "Test1");
    }

    #[test]
    fn inverted() -> OpmResult<()> {
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
    fn report() -> OpmResult<()> {
        let report = Dummy::default().node_report("123", crate::analyzers::AnalyzerKind::Energy)?;
        assert!(matches!(report, NodeReportResult::None));
        Ok(())
    }

    #[test]
    fn ports_inverted() -> OpmResult<()> {
        let mut node = Dummy::default();
        node.set_inverted(true)?;
        assert_eq!(node.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["input_1"]);
        Ok(())
    }

    #[test]
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<Dummy>()
    }

    #[test]
    fn analyze_wrong() -> OpmResult<()> {
        let mut dummy = Dummy::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_light);
        let output = AnalysisEnergy::analyze(&mut dummy, input, &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }

    #[test]
    fn analyze_ok() -> OpmResult<()> {
        let mut dummy = Dummy::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut dummy, input, &EnergyConfig::default())?;
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert_eq!(output, Some(&input_light));
        Ok(())
    }

    #[test]
    fn analyze_inverse() -> OpmResult<()> {
        let mut dummy = Dummy::default();
        dummy.set_inverted(true)?;
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut dummy, input, &EnergyConfig::default())?;
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert_eq!(output, Some(&input_light));
        Ok(())
    }

    #[test]
    fn analyze_raytrace_without_aperture() -> OpmResult<()> {
        let mut dummy = Dummy::default();
        dummy.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let ray = Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), nanometer!(1064.0), joule!(1.0))?;
        let rays = Rays::from(ray);

        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));

        // Raytracing must succeed without any apertures set on the dummy node
        let output = AnalysisRayTrace::analyze(&mut dummy, input, &RayTraceConfig::default())?;
        assert!(output.contains_key("output_1"));

        if let Some(LightData::Geometric(out_rays)) = output.get("output_1") {
            assert_eq!(out_rays.nr_of_rays(true), 1);
        } else {
            panic!("Expected geometric ray bundle in output port");
        }
        Ok(())
    }

    #[test]
    fn analyze_raytrace_with_aperture_apodization() -> OpmResult<()> {
        let mut dummy = Dummy::default();
        dummy.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;

        // Circular aperture with 5 mm radius
        let aperture = Aperture::new_circle(millimeter!(5.0), ApertureType::Hole, None)?;
        dummy.set_aperture(&PortType::Input, "input_1", &aperture)?;

        // Ray 1: Inside the aperture (r = 2 mm)
        let ray_inside =
            Ray::new_collimated(millimeter!(2.0, 0.0, 0.0), nanometer!(1064.0), joule!(1.0))?;
        // Ray 2: Outside the aperture (r = 7 mm)
        let ray_outside =
            Ray::new_collimated(millimeter!(7.0, 0.0, 0.0), nanometer!(1064.0), joule!(1.0))?;

        let rays = Rays::from(vec![ray_inside, ray_outside]);
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));

        let output = AnalysisRayTrace::analyze(&mut dummy, input, &RayTraceConfig::default())?;
        assert!(output.contains_key("output_1"));

        if let Some(LightData::Geometric(out_rays)) = output.get("output_1") {
            // Only the ray inside the aperture should survive as valid
            assert_eq!(out_rays.nr_of_rays(true), 1);
            // Total rays (valid + invalid) remains 2
            assert_eq!(out_rays.nr_of_rays(false), 2);
        } else {
            panic!("Expected geometric ray bundle in output port");
        }
        Ok(())
    }

    #[test]
    fn analyze_raytrace_inverted() -> OpmResult<()> {
        let mut dummy = Dummy::default();
        dummy.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        dummy.set_inverted(true)?;

        let ray = Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), nanometer!(1064.0), joule!(1.0))?;
        let rays = Rays::from(ray);

        let mut input = LightResult::default();
        input.insert("output_1".into(), LightData::Geometric(rays));

        let output = AnalysisRayTrace::analyze(&mut dummy, input, &RayTraceConfig::default())?;
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        Ok(())
    }
}
