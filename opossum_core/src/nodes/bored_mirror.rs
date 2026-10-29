#![warn(missing_docs)]
//! A mirror with a bore through it, such as a scraper mirror.
use super::splitter::Splitter;
use crate::{
    analyzers::{
        GhostFocusConfig, RayTraceConfig,
        energy::{AnalysisEnergy, EnergyConfig},
        ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    apertures::{Bore, CircleShape, OpticShape, Outline},
    coatings::CoatingConstantR,
    core_optics::{Appearance, NodeAttr, OpticNode, OpticNodeExt, SurfacePorts},
    degree,
    error::{OpmResult, OpossumError},
    geometry::{Extruded, Geometry, body::OPTIC_SHAPE},
    light::{LightRays, LightResult, Rays},
    material::{MATERIAL, Material},
    millimeter,
    nodes::NodeRegistration,
    percent,
    properties::{Proptype, validator::Validator},
    refractive_index::RefrIndexConst,
    utils::geom_transformation::Isometry,
};
use opm_macros_lib::OpmNode;
use uom::si::f64::Length;

inventory::submit! {
    NodeRegistration::new::<BoredMirror>("bored mirror", "mirror with a bore through it")
}

#[derive(OpmNode, Debug, Clone)]
#[opm_node("lightsteelblue")]
/// A mirror with a bore through it, such as a scraper mirror: a glass plate whose front face is
/// mirrored, with one bore through the plate.
///
/// The mirror reflects the light on its front face, while light in the bore passes the plate
/// untouched. It thus splits a beam in two, so it has the ports of a
/// [`BeamSplitter`](crate::nodes::BeamSplitter): what `input_1` sends through the bore leaves
/// `out1_trans1_refl2`, what the mirror reflects leaves `out2_trans2_refl1`. Light that enters the
/// glass, through the coating or at the rear face, is not traced through it yet: it is lost. Like a
/// beam splitter, the mirror is turned by its alignment (e.g. by 45°) to send the reflected light
/// sideways.
///
/// The optical axis is placed by the edge of the mirror alone: the mirror reflects it as if there
/// were no bore, and behind the plate it runs on through the center of the bore, parallel to the
/// arriving axis. An energy analysis knows no geometry, so it cannot split the light at the bore
/// and refuses the mirror.
///
/// Curvature convention of the front face:
/// - negative curvature will be a concave (focusing) mirror
/// - positive curvature will be a convex (defocusing) mirror
///
/// ## Optical Ports
///   - Inputs
///     - `input_1`, at the front face
///     - `input_2`, at the rear face
///   - Outputs
///     - `out1_trans1_refl2`, behind the plate
///     - `out2_trans2_refl1`, in front of the plate
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `curvature`: the radius of curvature of the mirrored front face
///   - `center thickness`: the thickness of the plate
///   - `material`: the glass of the plate
///   - `optic shape`: the edge of the mirror and the bore through it
///   - `front surface`: the coating of the mirror, fully reflecting by default, and its LIDT
///   - `rear surface`: the coating and the LIDT of the rear face
pub struct BoredMirror {
    node_attr: NodeAttr,
}

/// The optic shape a bored mirror starts out with: a circle of 12.5 mm radius, the default of
/// every component, with a bore of 2.5 mm radius through its center.
fn default_bored_shape() -> OpmResult<OpticShape> {
    Ok(Outline::new(
        CircleShape::new(millimeter!(12.5))?,
        Some(Bore::new(
            CircleShape::new(millimeter!(2.5))?,
            millimeter!(0.0, 0.0),
            degree!(0.0),
        )),
    )?
    .into())
}

impl Default for BoredMirror {
    /// Create a flat mirror on a glass plate of 6 mm, 25 mm across, with a bore of 5 mm through
    /// its center.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("bored mirror");
        node_attr
            .create_property_with_validator(
                "curvature",
                "radius of curvature of the mirrored front face",
                Validator::AndValidator {
                    validators: vec![Validator::NumericIsNotNaN, Validator::NumericIsNotZero],
                },
                Proptype::Curvature(millimeter!(f64::INFINITY)),
            )
            .unwrap();
        node_attr
            .create_property_with_validator(
                "center thickness",
                "thickness of the plate in the center",
                Validator::AndValidator {
                    validators: vec![Validator::NumericIsFinite, Validator::NumericIsPositive],
                },
                millimeter!(6.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                MATERIAL,
                "material of the plate",
                Material::new_draft(
                    "mirror substrate",
                    None,
                    None,
                    RefrIndexConst::new(1.5).unwrap().into(),
                )
                .into(),
            )
            .unwrap();
        node_attr
            .create_property_with_validator(
                OPTIC_SHAPE,
                "transversal extent of the component",
                Validator::OpticShapeIsBounded,
                default_bored_shape().unwrap().into(),
            )
            .unwrap();
        let mut mirror = Self { node_attr };
        mirror.create_default_surface_properties().unwrap();
        mirror
            .set_coating(
                Extruded::FRONT,
                &CoatingConstantR::new(percent!(100.0)).unwrap().into(),
            )
            .unwrap();
        mirror
    }
}
impl BoredMirror {
    /// Creates a new flat [`BoredMirror`] (see [`BoredMirror::default`]) with the given name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut mirror = Self::default();
        mirror.node_attr.set_name(name);
        mirror
    }
    /// Modifies the curvature of the mirrored front face of a [`BoredMirror`].
    ///
    /// A radius of curvature of +/- infinity corresponds to a flat face. This function can be used
    /// with the "builder pattern".
    ///
    /// # Errors
    ///
    /// This function will return an error if the given radius of curvature is zero or NaN, or if
    /// the face is curved too tightly to reach the edge of the mirror.
    pub fn with_curvature(mut self, curvature: Length) -> OpmResult<Self> {
        self.node_attr
            .set_property("curvature", Proptype::Curvature(curvature))?;
        self.update_surfaces()?;
        Ok(self)
    }
}
impl OpticNode for BoredMirror {
    fn create_geometry(&self) -> OpmResult<Option<Geometry>> {
        let (Ok(Proptype::Curvature(curvature)), Ok(Proptype::Length(center_thickness))) = (
            self.node_attr.get_property("curvature"),
            self.node_attr.get_property("center thickness"),
        ) else {
            return Err(OpossumError::Analysis(
                "cannot read the curvature and the center thickness of the bored mirror".into(),
            ));
        };
        let cross_section = self.cross_section()?.ok_or_else(|| {
            OpossumError::Analysis(format!(
                "the {OPTIC_SHAPE} of node '{}' is open, so its plate is unbounded",
                self.node_attr.name()
            ))
        })?;
        Ok(Some(Geometry::singlet(
            *curvature,
            millimeter!(f64::INFINITY),
            *center_thickness,
            cross_section,
            self.material()?,
        )?))
    }
    fn appearance(&self) -> Option<Appearance> {
        Some(Appearance::glass())
    }
    fn surface_ports(&self) -> SurfacePorts {
        SurfacePorts {
            inputs: &[("input_1", Extruded::FRONT), ("input_2", Extruded::REAR)],
            outputs: &[
                ("out1_trans1_refl2", Extruded::REAR),
                ("out2_trans2_refl1", Extruded::FRONT),
            ],
            splitting: &[],
        }
    }
}
impl Splitter for BoredMirror {}
impl AnalysisRayTrace for BoredMirror {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        self.split_rays(incoming_data, config)
    }
    fn calc_node_positions(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        self.split_axis(incoming_data, config)
    }
    fn axis_entrance_frame(&self, port_name: &str) -> Isometry {
        self.second_input_frame(port_name)
    }
}
impl AnalysisGhostFocus for BoredMirror {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        self.split_ghost_bundles(incoming_data, config)
    }
}
impl AnalysisEnergy for BoredMirror {
    /// An energy analysis knows no geometry, so it cannot tell the light in the bore from the
    /// light on the mirror: light reaching a bored mirror is refused.
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _config: &EnergyConfig,
    ) -> OpmResult<LightResult> {
        if incoming_data.is_empty() {
            return Ok(LightResult::default());
        }
        Err(OpossumError::Analysis(format!(
            "an energy analysis knows no geometry, so it cannot split the light at the bore of \
             bored mirror '{}': use a ray trace instead",
            self.node_attr.name()
        )))
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::energy::EnergyConfig,
        coatings::CoatingType,
        core_optics::{NodeAttrExt, PortType, node_attr::NodePositioning},
        joule,
        light::{LightData, Ray, spectrum_helper::create_he_ne_spec},
        nanometer,
        nodes::{
            Dummy, NodeGroup, SourcePort, round_collimated_ray_builder, test_helper::helper::*,
        },
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::Vector3;

    #[test]
    fn default() -> OpmResult<()> {
        let node = BoredMirror::default();
        assert_eq!(node.name(), "bored mirror");
        assert_eq!(node.node_type(), "bored mirror");
        assert_eq!(node.node_color(), "lightsteelblue");
        assert!(!node.inverted());
        assert_eq!(
            node.node_attr.get_property("center thickness")?,
            &Proptype::Length(millimeter!(6.0))
        );
        assert_eq!(
            node.node_attr.get_property(OPTIC_SHAPE)?,
            &Proptype::OpticShape(default_bored_shape()?)
        );
        let front = node.face_name(Extruded::FRONT)?;
        assert_eq!(
            node.surface_config(&front)?.coating(),
            &CoatingType::from(CoatingConstantR::new(percent!(100.0))?)
        );
        Ok(())
    }
    #[test]
    fn inverted() -> OpmResult<()> {
        test_inverted::<BoredMirror>()
    }
    #[test]
    fn ports() -> OpmResult<()> {
        let mut node = BoredMirror::default();
        let sorted = |node: &BoredMirror, port_type| {
            let mut names = node.ports().names(&port_type);
            names.sort();
            names
        };
        assert_eq!(sorted(&node, PortType::Input), ["input_1", "input_2"]);
        assert_eq!(
            sorted(&node, PortType::Output),
            ["out1_trans1_refl2", "out2_trans2_refl1"]
        );
        node.set_inverted(true)?;
        assert_eq!(
            sorted(&node, PortType::Input),
            ["out1_trans1_refl2", "out2_trans2_refl1"]
        );
        Ok(())
    }
    /// Send rays into an input of the mirror in a ray trace.
    ///
    /// # Returns
    ///
    /// The bundles leaving the first and the second output.
    fn ray_trace(mirror: &mut BoredMirror, input: &str, rays: Rays) -> OpmResult<(Rays, Rays)> {
        let output = AnalysisRayTrace::analyze(
            mirror,
            LightResult::from([(input.into(), LightData::Geometric(rays))]),
            &RayTraceConfig::default(),
        )?;
        let bundle = |port: &str| match output.get(port) {
            Some(LightData::Geometric(rays)) => rays.clone(),
            _ => panic!("expected rays at port {port}"),
        };
        Ok((bundle("out1_trans1_refl2"), bundle("out2_trans2_refl1")))
    }
    /// Assert whether a warning holding the given text was logged since `testing_logger::setup`.
    fn assert_warning(text: &str, logged: bool) {
        testing_logger::validate(|logs| {
            let warnings = logs
                .iter()
                .filter(|log| log.level == log::Level::Warn)
                .map(|log| log.body.as_str())
                .collect::<Vec<_>>();
            assert_eq!(
                warnings.iter().any(|warning| warning.contains(text)),
                logged,
                "{warnings:?}"
            );
        });
    }
    /// Light on the mirror is reflected, light in the bore passes the plate untouched. The part
    /// the mirror transmits into the glass carries no energy, so its loss is not worth a warning.
    #[test]
    fn the_mirror_reflects_and_the_bore_lets_pass() -> OpmResult<()> {
        let mut mirror = placed_at_origin::<BoredMirror>()?;
        let rays = rays_along_z_from(
            &[millimeter!(0.0, 0.0, -10.0), millimeter!(0.0, 5.0, -10.0)],
            nanometer!(1000.0),
        )?;
        testing_logger::setup();
        let (behind, in_front) = ray_trace(&mut mirror, "input_1", rays)?;
        assert_warning("enters its body", false);
        assert_valid_energies(&behind, &[1.0], "the ray through the bore passes");
        assert_valid_energies(&in_front, &[1.0], "the ray on the mirror is reflected");
        let through = behind.iter().find(|ray| ray.valid()).expect("one ray");
        assert_eq!(through.position(), millimeter!(0.0, 0.0, -10.0));
        assert_eq!(through.direction(), Vector3::z());
        let back = in_front.iter().find(|ray| ray.valid()).expect("one ray");
        assert_abs_diff_eq!(back.direction(), -Vector3::z(), epsilon = 1e-12);
        Ok(())
    }
    /// Light arriving from behind passes the bore into the second output. Light on the rear face
    /// enters the glass, which is not traced yet: it is lost, with a warning.
    #[test]
    fn light_from_behind_on_the_glass_is_lost_with_a_warning() -> OpmResult<()> {
        let mut mirror = placed_at_origin::<BoredMirror>()?;
        let from_behind = |y: f64| {
            Ray::new(
                millimeter!(0.0, y, 20.0),
                -Vector3::z(),
                nanometer!(1000.0),
                joule!(1.0),
            )
        };
        let rays = Rays::from(vec![from_behind(0.0)?, from_behind(5.0)?]);
        testing_logger::setup();
        let (behind, in_front) = ray_trace(&mut mirror, "input_2", rays)?;
        assert_warning("enters its body", true);
        assert_valid_energies(&in_front, &[1.0], "the ray through the bore passes");
        assert_valid_energies(&behind, &[], "the rear face reflects nothing");
        Ok(())
    }
    /// A concave mirror focuses what it reflects: a ray 5 mm off the axis of a mirror with a
    /// radius of curvature of 200 mm crosses the axis near its focal point, 100 mm in front of it.
    #[test]
    fn a_concave_bored_mirror_focuses_what_it_reflects() -> OpmResult<()> {
        let mut mirror = BoredMirror::default().with_curvature(millimeter!(-200.0))?;
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let rays = rays_along_z_from(&[millimeter!(0.0, 5.0, -10.0)], nanometer!(1000.0))?;
        let (_, in_front) = ray_trace(&mut mirror, "input_1", rays)?;
        let back = in_front.iter().find(|ray| ray.valid()).expect("one ray");
        let (position, direction) = (back.position(), back.direction());
        let across = -position.y.value / direction.y;
        let crossing = direction.z.mul_add(across, position.z.value);
        assert_abs_diff_eq!(crossing, -0.1, epsilon = 1e-3);
        Ok(())
    }
    /// The world position (in meters) and facing direction of a placed node.
    fn placed(scenery: &NodeGroup, uuid: uuid::Uuid) -> OpmResult<(Vector3<f64>, Vector3<f64>)> {
        let iso = scenery
            .node(uuid)?
            .effective_position()
            .cloned()
            .ok_or_else(|| OpossumError::Other("the node was not placed".into()))?;
        let t = iso.translation();
        Ok((
            Vector3::new(t.x.value, t.y.value, t.z.value),
            iso.transform_vector_f64(&Vector3::z()),
        ))
    }
    /// The optical axis is placed by the edge of a 45° mirror 100 mm from the source: the mirror
    /// reflects it at its front face as if there were no bore, and behind the plate it runs on
    /// through the center of the bore, where that line leaves the rear face. On the axis, the
    /// tilted rear face lies 6 mm / cos 45° = 8.485 mm behind the front face, so the node 50 mm
    /// behind the plate sits at 158.485 mm.
    #[test]
    fn the_axis_is_placed_by_the_edge_and_leaves_through_the_bore() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let src = scenery.add_node(SourcePort::default())?;
        let mut mirror = BoredMirror::default();
        mirror.set_alignment(millimeter!(0., 0., 0.), degree!(0., 45., 0.))?;
        let mirror = scenery.add_node(mirror)?;
        let behind = scenery.add_node(Dummy::new("behind"))?;
        let reflected = scenery.add_node(Dummy::new("reflected"))?;
        scenery.connect_nodes(src, "output_1", mirror, "input_1", millimeter!(100.0))?;
        scenery.connect_nodes(
            mirror,
            "out1_trans1_refl2",
            behind,
            "input_1",
            millimeter!(50.0),
        )?;
        scenery.connect_nodes(
            mirror,
            "out2_trans2_refl1",
            reflected,
            "input_1",
            millimeter!(50.0),
        )?;
        let mut config = RayTraceConfig::default();
        config.map_source(
            src,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?,
        );
        AnalysisRayTrace::calc_node_positions(
            &mut scenery,
            LightResult::default(),
            &config.for_positioning(),
        )?;
        let millimeters = |x: f64, y: f64, z: f64| Vector3::new(x, y, z) / 1000.0;
        let rear = 6.0 / std::f64::consts::FRAC_1_SQRT_2;
        let (behind_pos, behind_dir) = placed(&scenery, behind)?;
        assert_abs_diff_eq!(behind_pos, millimeters(0., 0., 150. + rear), epsilon = 1e-9);
        assert_abs_diff_eq!(behind_dir, Vector3::z(), epsilon = 1e-9);
        let (reflected_pos, reflected_dir) = placed(&scenery, reflected)?;
        assert_abs_diff_eq!(reflected_pos, millimeters(-50., 0., 100.), epsilon = 1e-9);
        assert_abs_diff_eq!(reflected_dir, -Vector3::x(), epsilon = 1e-9);
        Ok(())
    }
    /// An energy analysis cannot split the light at the bore, so it refuses light reaching the
    /// mirror and names it; without light there is nothing to refuse.
    #[test]
    fn an_energy_analysis_refuses_the_mirror() -> OpmResult<()> {
        let mut mirror = BoredMirror::new("scraper");
        let input =
            LightResult::from([("input_1".into(), LightData::Energy(create_he_ne_spec(1.0)?))]);
        let error = AnalysisEnergy::analyze(&mut mirror, input, &EnergyConfig::default())
            .expect_err("the energy analysis split the light at the bore");
        assert!(error.to_string().contains("'scraper'"), "{error}");
        let nothing = AnalysisEnergy::analyze(
            &mut mirror,
            LightResult::default(),
            &EnergyConfig::default(),
        )?;
        assert!(nothing.is_empty());
        Ok(())
    }
}
