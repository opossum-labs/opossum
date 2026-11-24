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
    core_optics::{NodeAttr, OpticNode, OpticNodeExt, SurfacePorts},
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
use nalgebra::Vector3;
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
/// untouched. It thus splits a beam in two, like a [`BeamSplitter`](crate::nodes::BeamSplitter):
/// what `mirror` sends through the bore leaves `behind`, what the mirror reflects leaves
/// `in_front`, together with what `through_bore` sends through the bore from behind. Light that
/// enters the glass, through the coating or at the rear face, is not traced through it yet: it is
/// lost. Like a beam splitter, the mirror is turned by its alignment (e.g. by 45°) to send the
/// reflected light sideways.
///
/// The optical axis is placed by the edge of the mirror alone: the mirror reflects it as if there
/// were no bore, and behind the plate it runs on through the center of the bore, parallel to the
/// arriving axis. Light at `through_bore` only passes the bore, so a mirror placed through it gets
/// the axis from behind along the bore, through its center on the rear face. An energy analysis
/// knows no geometry, so it cannot split the light at the bore and refuses the mirror.
///
/// Curvature convention of the front face:
/// - negative curvature will be a concave (focusing) mirror
/// - positive curvature will be a convex (defocusing) mirror
///
/// ## Optical Ports
///   - Inputs
///     - `mirror`, at the mirrored front face
///     - `through_bore`, at the rear face, for light that passes the bore from behind
///   - Outputs
///     - `behind`, behind the plate
///     - `in_front`, in front of the plate
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
                "radius of curvature",
                "radius of curvature of the mirrored front face",
                Validator::AndValidator {
                    validators: vec![Validator::NumericIsNotNaN, Validator::NumericIsNotZero],
                },
                Proptype::RadiusOfCurvature(millimeter!(f64::INFINITY)),
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
                Validator::OpticShape {
                    may_be_open: false,
                    bored: true,
                },
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
        self.node_attr.set_property(
            "radius of curvature",
            Proptype::RadiusOfCurvature(curvature),
        )?;
        self.update_surfaces()?;
        Ok(self)
    }
}
impl OpticNode for BoredMirror {
    fn create_geometry(&self) -> OpmResult<Option<Geometry>> {
        let (Ok(Proptype::RadiusOfCurvature(curvature)), Ok(Proptype::Length(center_thickness))) = (
            self.node_attr.get_property("radius of curvature"),
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
    fn surface_ports(&self) -> SurfacePorts {
        SurfacePorts {
            inputs: &[
                ("mirror", Extruded::FRONT),
                ("through_bore", Extruded::REAR),
            ],
            outputs: &[("behind", Extruded::REAR), ("in_front", Extruded::FRONT)],
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
    /// Return the local frame the optical axis enters the given input port from (see
    /// [`AnalysisRayTrace::axis_entrance_frame`]).
    ///
    /// At the front face the axis enters as at every node, along +z through the origin. At the
    /// rear face light only passes the bore, so the axis enters from behind along the axis of the
    /// bore, through its center on the rear face, towards its center on the front face. The
    /// alignment of the node turns and shifts that frame with the mirror.
    ///
    /// # Errors
    ///
    /// This function returns an error if the port is not one of the mirror's, or if the mirror
    /// has no bore for the axis to enter through.
    fn axis_entrance_frame(&self, port_name: &str) -> OpmResult<Isometry> {
        let face = self.face_of_port(port_name)?;
        if face != Extruded::REAR {
            return Ok(Isometry::identity());
        }
        let no_bore = || {
            OpossumError::Analysis(format!(
                "bored mirror '{}' has no bore for the optical axis to enter through",
                self.node_attr.name()
            ))
        };
        let geometry = self.create_geometry()?.ok_or_else(no_bore)?;
        let rim = geometry.kind().bored_rim(face).ok_or_else(no_bore)?;
        let center_on = |at| -> OpmResult<_> {
            rim.bore_center_on(geometry.face(at).ok_or_else(no_bore)?)?
                .ok_or_else(no_bore)
        };
        let (entry, exit) = (center_on(Extruded::REAR)?, center_on(Extruded::FRONT)?);
        let through = (exit - entry).map(|length| length.value);
        let alignment = (*self.node_attr.alignment()).unwrap_or_else(Isometry::identity);
        Ok(alignment.append(&Isometry::new_from_view(entry, through, Vector3::y())))
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
        analyzers::{Analyzer, energy::EnergyConfig, raytrace::RayTracingAnalyzer},
        coatings::CoatingType,
        core_optics::{NodeAttrExt, PortType, node_attr::NodePositioning},
        joule,
        light::{LightData, Ray, spectrum_helper::create_he_ne_spec},
        nanometer,
        nodes::{
            Dummy, EnergyMeter, NodeGroup, SourcePort, round_collimated_ray_builder,
            test_helper::helper::*,
        },
    };
    use approx::assert_abs_diff_eq;
    use std::f64::consts::FRAC_1_SQRT_2;

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
        assert_eq!(sorted(&node, PortType::Input), ["mirror", "through_bore"]);
        assert_eq!(sorted(&node, PortType::Output), ["behind", "in_front"]);
        node.set_inverted(true)?;
        assert_eq!(sorted(&node, PortType::Input), ["behind", "in_front"]);
        Ok(())
    }
    /// Send rays into an input of the mirror in a ray trace.
    ///
    /// # Returns
    ///
    /// The bundles leaving behind and in front of the plate.
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
        Ok((bundle("behind"), bundle("in_front")))
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
        let (behind, in_front) = ray_trace(&mut mirror, "mirror", rays)?;
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
    /// Light arriving from behind passes the bore and leaves in front of the plate. Light on the
    /// rear face enters the glass, which is not traced yet: it is lost, with a warning.
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
        let (behind, in_front) = ray_trace(&mut mirror, "through_bore", rays)?;
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
        let (_, in_front) = ray_trace(&mut mirror, "mirror", rays)?;
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
    /// A default bored mirror with its bore moved to the given x in mm of its own plane and
    /// tilted by the given angle in degrees towards +x.
    fn bored_at(x: f64, towards_x: f64) -> OpmResult<BoredMirror> {
        let mut mirror = BoredMirror::default();
        let outline = Outline::new(
            CircleShape::new(millimeter!(12.5))?,
            Some(
                Bore::new(
                    CircleShape::new(millimeter!(2.5))?,
                    millimeter!(x, 0.0),
                    degree!(0.0),
                )
                .with_tilt(degree!(0.0), degree!(towards_x)),
            ),
        )?;
        mirror.set_property(OPTIC_SHAPE, OpticShape::from(outline).into())?;
        Ok(mirror)
    }
    /// Place the given mirror, turned by 45° about y, 100 mm from a source along z, and a dummy
    /// 50 mm behind each of its outputs.
    ///
    /// # Returns
    ///
    /// Where the dummies behind the plate and in the reflected beam are placed, in meters, each
    /// with the direction it faces.
    #[allow(clippy::type_complexity)]
    fn behind_a_tilted(
        mut mirror: BoredMirror,
    ) -> OpmResult<((Vector3<f64>, Vector3<f64>), (Vector3<f64>, Vector3<f64>))> {
        let mut scenery = NodeGroup::default();
        let src = scenery.add_node(SourcePort::default())?;
        mirror.set_alignment(millimeter!(0., 0., 0.), degree!(0., 45., 0.))?;
        let mirror = scenery.add_node(mirror)?;
        let behind = scenery.add_node(Dummy::new("behind"))?;
        let reflected = scenery.add_node(Dummy::new("reflected"))?;
        scenery.connect_nodes(src, "output_1", mirror, "mirror", millimeter!(100.0))?;
        scenery.connect_nodes(mirror, "behind", behind, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(mirror, "in_front", reflected, "input_1", millimeter!(50.0))?;
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
        Ok((placed(&scenery, behind)?, placed(&scenery, reflected)?))
    }
    /// A position given in millimeters, in meters.
    fn millimeters(x: f64, y: f64, z: f64) -> Vector3<f64> {
        Vector3::new(x, y, z) / 1000.0
    }
    /// The optical axis is placed by the edge of a 45° mirror 100 mm from the source: the mirror
    /// reflects it at its front face as if there were no bore, and behind the plate it runs on
    /// through the center of the bore, where that line leaves the rear face. On the axis, the
    /// tilted rear face lies 6 mm / cos 45° = 8.485 mm behind the front face, so the node 50 mm
    /// behind the plate sits at 158.485 mm.
    #[test]
    fn the_axis_is_placed_by_the_edge_and_leaves_through_the_bore() -> OpmResult<()> {
        let ((behind_pos, behind_dir), (reflected_pos, reflected_dir)) =
            behind_a_tilted(BoredMirror::default())?;
        let rear = 6.0 / std::f64::consts::FRAC_1_SQRT_2;
        assert_abs_diff_eq!(behind_pos, millimeters(0., 0., 150. + rear), epsilon = 1e-9);
        assert_abs_diff_eq!(behind_dir, Vector3::z(), epsilon = 1e-9);
        assert_abs_diff_eq!(reflected_pos, millimeters(-50., 0., 100.), epsilon = 1e-9);
        assert_abs_diff_eq!(reflected_dir, -Vector3::x(), epsilon = 1e-9);
        Ok(())
    }
    /// With the bore 5 mm beside the axis, the light it lets pass runs on parallel to the axis
    /// through the center of the bore. The mirror turned by 45° about y carries its own x axis to
    /// (cos 45°, 0, −sin 45°), which puts the center at (3.536, 0, 96.464) mm on the front face;
    /// the line leaves the rear face 6 mm / cos 45° = 8.485 mm further along z, and the node 50 mm
    /// behind it sits at z = 154.950 mm. The reflected branch stays as if there were no bore.
    #[test]
    fn the_axis_behind_the_plate_runs_through_a_bore_beside_it() -> OpmResult<()> {
        let ((behind_pos, behind_dir), (reflected_pos, reflected_dir)) =
            behind_a_tilted(bored_at(5.0, 0.0)?)?;
        let (side, rear) = (
            5.0 * std::f64::consts::FRAC_1_SQRT_2,
            6.0 / std::f64::consts::FRAC_1_SQRT_2,
        );
        assert_abs_diff_eq!(
            behind_pos,
            millimeters(side, 0., 150. - side + rear),
            epsilon = 1e-9
        );
        assert_abs_diff_eq!(behind_dir, Vector3::z(), epsilon = 1e-9);
        assert_abs_diff_eq!(reflected_pos, millimeters(-50., 0., 100.), epsilon = 1e-9);
        assert_abs_diff_eq!(reflected_dir, -Vector3::x(), epsilon = 1e-9);
        Ok(())
    }
    /// Behind the bore, the optical axis starts where it meets the plate, in the bore on the side
    /// it arrives from, and runs through the bore to the rear face: the drawn axis leaves no gap
    /// across the plate, nor a line across it from where the arriving axis met it to a bore beside
    /// it.
    #[test]
    fn the_axis_runs_through_the_bore_from_where_it_meets_the_plate() -> OpmResult<()> {
        let mut mirror = bored_at(5.0, 0.0)?;
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let axis = rays_along_z_from(&[millimeter!(0.0, 0.0, -10.0)], nanometer!(1000.0))?;
        let output = AnalysisRayTrace::calc_node_positions(
            &mut mirror,
            LightResult::from([("mirror".into(), LightData::Geometric(axis))]),
            &RayTraceConfig::default(),
        )?;
        let Some(LightData::Geometric(behind)) = output.get("behind") else {
            panic!("expected the axis behind the plate");
        };
        let path = behind
            .iter()
            .next()
            .expect("one axis ray")
            .position_history_with_current();
        assert_eq!(path.nrows(), 2, "{path:?}");
        for (row, z) in [(0, 0.0), (1, 6.0)] {
            assert_eq!(
                (path[(row, 0)], path[(row, 1)], path[(row, 2)]),
                (millimeter!(5.0), millimeter!(0.0), millimeter!(z)),
                "point {row}"
            );
        }
        Ok(())
    }
    /// Light in a bore tilted against it may hit the wall of the bore before it leaves the plate:
    /// that light is lost, with a warning. The bore of 2.5 mm radius is tilted by 30° towards −x,
    /// so on the rear face, 6 mm deep, its axis lies 6 mm · tan 30° = 3.464 mm towards −x. A ray
    /// along z through the center of the bore on the front face lies 3.464 mm · cos 30° = 3.0 mm
    /// from that axis there and hits the wall; one 1.5 mm towards −x stays 1.70 mm from it and
    /// passes; one 6 mm towards +x meets the mirror and is reflected.
    #[test]
    fn light_that_hits_the_wall_of_the_bore_is_lost_with_a_warning() -> OpmResult<()> {
        let mut mirror = bored_at(0.0, -30.0)?;
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let rays = rays_along_z_from(
            &[
                millimeter!(0.0, 0.0, -10.0),
                millimeter!(-1.5, 0.0, -10.0),
                millimeter!(6.0, 0.0, -10.0),
            ],
            nanometer!(1000.0),
        )?;
        testing_logger::setup();
        let (behind, in_front) = ray_trace(&mut mirror, "mirror", rays)?;
        assert_warning("hits the wall of the bore", true);
        assert_valid_energies(
            &behind,
            &[1.0],
            "only the ray that stays in the bore passes",
        );
        assert_valid_energies(&in_front, &[1.0], "the ray on the mirror is reflected");
        let through = behind.iter().find(|ray| ray.valid()).expect("one ray");
        assert_eq!(through.position(), millimeter!(-1.5, 0.0, -10.0));
        Ok(())
    }
    /// Arriving from behind, the axis passes a tilted bore from where the axis of the bore meets
    /// the rear face, 6 mm · tan 30° = 3.464 mm towards −x for a bore tilted by 30° towards −x, and
    /// runs on along the arriving axis to the front face.
    #[test]
    fn from_behind_the_axis_passes_the_bore_from_its_center_on_the_rear_face() -> OpmResult<()> {
        let mut mirror = bored_at(0.0, -30.0)?;
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let axis = Rays::from(Ray::new(
            millimeter!(0.0, 0.0, 20.0),
            -Vector3::z(),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        let output = AnalysisRayTrace::calc_node_positions(
            &mut mirror,
            LightResult::from([("through_bore".into(), LightData::Geometric(axis))]),
            &RayTraceConfig::default(),
        )?;
        let Some(LightData::Geometric(in_front)) = output.get("in_front") else {
            panic!("expected the axis in front of the plate");
        };
        let path = in_front
            .iter()
            .next()
            .expect("one axis ray")
            .position_history_with_current();
        assert_eq!(path.nrows(), 2, "{path:?}");
        let side = -6.0 * 30.0_f64.to_radians().tan();
        for (row, z) in [(0, 6.0), (1, 0.0)] {
            assert_abs_diff_eq!(path[(row, 0)].value, side / 1000.0, epsilon = 1e-12);
            assert_abs_diff_eq!(path[(row, 2)].value, z / 1000.0, epsilon = 1e-12);
        }
        Ok(())
    }
    /// A mirror turned by 45° about y, with its bore 5 mm beside its center tilted by 45° towards
    /// −x, so that the bore runs along the arriving beam.
    fn a_scraper_along_the_beam() -> OpmResult<BoredMirror> {
        let mut mirror = bored_at(5.0, -45.0)?;
        mirror.set_alignment(millimeter!(0., 0., 0.), degree!(0., 45., 0.))?;
        Ok(mirror)
    }
    /// `through_bore` lies on the rear face, where light only passes the bore: its axis enters
    /// from behind along the axis of the bore, through the center of the bore on the rear face.
    /// On [`a_scraper_along_the_beam`] that center lies at (−1, 0, 6) mm of the mirror's own frame,
    /// which the turn carries to (3.536, 0, 4.950) mm, and the axis enters along −z. `mirror`
    /// keeps the axis of every node. Inverted, the port on the rear face, `behind`, enters the same
    /// way and the one on the front face, `in_front`, like `mirror`.
    #[test]
    fn the_bore_input_enters_along_the_bore_from_behind() -> OpmResult<()> {
        let mut mirror = a_scraper_along_the_beam()?;
        let through_the_bore = mirror.axis_entrance_frame("through_bore")?;
        let entry = through_the_bore.translation();
        assert_abs_diff_eq!(
            Vector3::new(entry.x.value, entry.y.value, entry.z.value),
            millimeters(5.0 * FRAC_1_SQRT_2, 0.0, 7.0 * FRAC_1_SQRT_2),
            epsilon = 1e-12
        );
        for (local, placed) in [(Vector3::z(), -Vector3::z()), (Vector3::y(), Vector3::y())] {
            assert_abs_diff_eq!(
                through_the_bore.transform_vector_f64(&local),
                placed,
                epsilon = 1e-12
            );
        }
        assert_eq!(mirror.axis_entrance_frame("mirror")?, Isometry::identity());
        mirror.set_inverted(true)?;
        assert_eq!(mirror.axis_entrance_frame("behind")?, through_the_bore);
        assert_eq!(
            mirror.axis_entrance_frame("in_front")?,
            Isometry::identity()
        );
        Ok(())
    }
    /// A mirror shifted by its alignment takes the entrance of its bore along: the axis still
    /// enters through the center of the bore, 2 mm further along y.
    #[test]
    fn a_shifted_mirror_takes_the_entrance_of_its_bore_along() -> OpmResult<()> {
        let mut mirror = BoredMirror::default();
        let upright = mirror.axis_entrance_frame("through_bore")?;
        mirror.set_alignment(millimeter!(0., 2., 0.), degree!(0., 0., 0.))?;
        let shifted = mirror.axis_entrance_frame("through_bore")?;
        let (from, to) = (upright.translation(), shifted.translation());
        assert_abs_diff_eq!(
            Vector3::new(
                (to.x - from.x).value,
                (to.y - from.y).value,
                (to.z - from.z).value
            ),
            millimeters(0.0, 2.0, 0.0),
            epsilon = 1e-15
        );
        Ok(())
    }
    /// Place the given mirror 100 mm from a source along z, fed through `through_bore`, with an
    /// energy meter 50 mm in front of it, and trace a collimated beam of 1 mm radius through it.
    ///
    /// # Returns
    ///
    /// Where the meter is placed, in meters, the direction it faces and how many of the 7 rays of
    /// the beam it records.
    fn through_the_bore(mirror: BoredMirror) -> OpmResult<(Vector3<f64>, Vector3<f64>, usize)> {
        let mut scenery = NodeGroup::default();
        let src = scenery.add_node(SourcePort::default())?;
        let mirror = scenery.add_node(mirror)?;
        let meter = scenery.add_node(EnergyMeter::default())?;
        scenery.connect_nodes(src, "output_1", mirror, "through_bore", millimeter!(100.0))?;
        scenery.connect_nodes(mirror, "in_front", meter, "input_1", millimeter!(50.0))?;
        let mut config = RayTraceConfig::default();
        config.map_source(
            src,
            round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 1)?,
        );
        testing_logger::setup();
        RayTracingAnalyzer::new(config).analyze(&mut scenery)?;
        assert_warning("that light is lost", false);
        let (position, direction) = placed(&scenery, meter)?;
        Ok((
            position,
            direction,
            rays_recorded_by_meter(&scenery, meter)?,
        ))
    }
    /// Placed through `through_bore`, the mirror lets the light pass its bore from behind,
    /// along the axis of the bore: none of it is lost on the glass or on the wall of the bore, and
    /// the node in front of the mirror is placed along the bore. The distance from the source counts
    /// to where the axis meets the rear face. On a default mirror the front face lies 6 mm further
    /// on, so the meter 50 mm in front of it sits at 156 mm. Through [`a_scraper_along_the_beam`]
    /// the axis runs 6 mm / cos 45° = 8.485 mm, so the meter sits at 158.485 mm.
    #[test]
    fn placed_through_the_bore_input_the_light_passes_the_bore() -> OpmResult<()> {
        for (mirror, through) in [
            (BoredMirror::default(), 6.0),
            (a_scraper_along_the_beam()?, 6.0 / FRAC_1_SQRT_2),
        ] {
            let (position, direction, recorded) = through_the_bore(mirror)?;
            assert_abs_diff_eq!(
                position,
                millimeters(0.0, 0.0, 150.0 + through),
                epsilon = 1e-9
            );
            assert_abs_diff_eq!(direction, Vector3::z(), epsilon = 1e-9);
            assert_eq!(recorded, 7, "every ray of the beam passes the bore");
        }
        Ok(())
    }
    /// With light on both inputs, `mirror` places the mirror. A source left to the positioning run
    /// on `through_bore` is put behind the bore, aimed along its axis, 80 mm from its center
    /// on the rear face. [`a_scraper_along_the_beam`] placed 100 mm from the first source has that
    /// center at (3.536, 0, 104.950) mm. The light of both sources meets the mirror without a
    /// warning.
    #[test]
    fn a_source_feeding_the_bore_input_is_placed_behind_the_bore() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let first = scenery.add_node(SourcePort::default())?;
        let mut behind = SourcePort::new("behind");
        behind.set_positioning(NodePositioning::Automatic(None))?;
        let behind = scenery.add_node(behind)?;
        let mirror = scenery.add_node(a_scraper_along_the_beam()?)?;
        scenery.connect_nodes(first, "output_1", mirror, "mirror", millimeter!(100.0))?;
        scenery.connect_nodes(
            behind,
            "output_1",
            mirror,
            "through_bore",
            millimeter!(80.0),
        )?;
        let mut config = RayTraceConfig::default();
        for source in [first, behind] {
            config.map_source(
                source,
                round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 1)?,
            );
        }
        testing_logger::setup();
        AnalysisRayTrace::calc_node_positions(
            &mut scenery,
            LightResult::default(),
            &config.for_positioning(),
        )?;
        testing_logger::validate(|logs| {
            let warnings = logs
                .iter()
                .filter(|log| log.level == log::Level::Warn)
                .map(|log| log.body.as_str())
                .collect::<Vec<_>>();
            assert!(warnings.is_empty(), "{warnings:?}");
        });
        let (mirror_position, _) = placed(&scenery, mirror)?;
        assert_abs_diff_eq!(mirror_position, millimeters(0., 0., 100.), epsilon = 1e-9);
        let (position, direction) = placed(&scenery, behind)?;
        assert_abs_diff_eq!(
            position,
            millimeters(
                5.0 * FRAC_1_SQRT_2,
                0.0,
                7.0f64.mul_add(FRAC_1_SQRT_2, 180.0)
            ),
            epsilon = 1e-9
        );
        assert_abs_diff_eq!(direction, -Vector3::z(), epsilon = 1e-9);
        Ok(())
    }
    /// An energy analysis cannot split the light at the bore, so it refuses light reaching the
    /// mirror and names it; without light there is nothing to refuse.
    #[test]
    fn an_energy_analysis_refuses_the_mirror() -> OpmResult<()> {
        let mut mirror = BoredMirror::new("scraper");
        let input =
            LightResult::from([("mirror".into(), LightData::Energy(create_he_ne_spec(1.0)?))]);
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
