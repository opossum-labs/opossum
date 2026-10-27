#![warn(missing_docs)]
//! A fake / dummy optical element without active modification of incoming light.

use opm_macros_lib::OpmNode;

use crate::{
    analyzers::{
        RayTraceConfig, energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    apertures::OpticShape,
    core_optics::{
        Appearance, NodeAttr, OpticNode, OpticNodeExt, SurfacePorts,
        optic_node_ext::single_io_port_names,
    },
    error::OpmResult,
    geometry::{Geometry, body::OPTIC_SHAPE},
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
/// A [`Dummy`] is unbounded by default; given an optic shape it acts as an iris, which a ray outside
/// misses.
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
///   - `optic shape`: the extent of the surface, open by default; a ray beyond it misses the
///     dummy and follows the analyzer's missed surface strategy
pub struct Dummy {
    node_attr: NodeAttr,
}

impl Default for Dummy {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("dummy");
        node_attr
            .create_property(
                OPTIC_SHAPE,
                "transversal extent of the surface, open for an unbounded one",
                OpticShape::Open.into(),
            )
            .expect("Failed to declare dummy properties");
        let mut dummy = Self { node_attr };
        dummy
            .create_default_surface_properties()
            .expect("Failed to declare dummy surface properties");
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
        self.unified_analyze_single_surface_node(incoming_data, config, &in_port)
    }
}

impl OpticNode for Dummy {
    fn create_geometry(&self) -> OpmResult<Option<Geometry>> {
        Ok(Some(Geometry::plane(self.cross_section()?)))
    }
    /// A dummy is only traced at; it is not drawn.
    fn appearance(&self) -> Option<Appearance> {
        None
    }
    fn surface_ports(&self) -> SurfacePorts {
        SurfacePorts::SINGLE_SURFACE
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::energy::EnergyConfig,
        core_optics::{NodeAttrExt, PortType, node_attr::NodePositioning},
        joule,
        light::{LightData, Ray, Rays, spectrum_helper::create_he_ne_spec},
        millimeter, nanometer,
        nodes::test_helper::helper::*,
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

    /// Bounded by an optic shape, a dummy is an iris: a ray outside it misses the dummy.
    #[test]
    fn a_bounded_dummy_is_an_iris() -> OpmResult<()> {
        test_rays_beyond_optic_shape_are_lost::<Dummy>()
    }

    /// Unbounded by default, a dummy passes every ray, however far off its axis.
    #[test]
    fn a_dummy_is_unbounded_by_default() -> OpmResult<()> {
        let rays = rays_along_z_from(&[millimeter!(0.0, 100.0, -10.0)], nanometer!(1000.0))?;
        let output = AnalysisRayTrace::analyze(
            &mut placed_at_origin::<Dummy>()?,
            LightResult::from([("input_1".into(), LightData::Geometric(rays))]),
            &RayTraceConfig::default(),
        )?;
        let Some(LightData::Geometric(passed)) = output.get("output_1") else {
            panic!("expected ray data at the output port");
        };
        assert_eq!(passed.nr_of_rays(true), 1);
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
