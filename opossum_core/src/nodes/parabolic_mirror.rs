use crate::{
    analyzers::{
        GhostFocusConfig, RayTraceConfig,
        energy::AnalysisEnergy,
        ghostfocus::AnalysisGhostFocus,
        propagation_strategy::{MissedSurfaceStrategy, PropagationStrategy},
        raytrace::AnalysisRayTrace,
    },
    coatings::CoatingConstantR,
    core_optics::{
        NodeAttr, NodeAttrExt, OpticNode, OpticNodeExt, PortType, node_attr::HasNodeAttr,
    },
    degree,
    error::{OpmResult, OpossumError},
    geometry::{Geometry, SurfaceShape},
    light::{LightData, LightRays, LightResult, Rays},
    meter,
    nodes::{NodeRegistration, create_surface_properties},
    percent,
    properties::{Proptype, validator::Validator},
    radian,
    utils::geom_transformation::Isometry,
};
use nalgebra::{Isometry3, Point3, Vector2, Vector3, vector};
use opm_macros_lib::OpmNode;
use uom::si::f64::{Angle, Length};

inventory::submit! {
    NodeRegistration::new::<ParabolicMirror>("parabolic mirror", "parabolic mirror")
}

#[derive(OpmNode, Debug, Clone)]
#[opm_node("chocolate2")]
/// An infinitely thin mirror with a spherical (or flat) surface.
///
/// # Focal length convention:
/// - positive focal length will be a common focusing parabola
/// - negative focal length will be a defocusing parabola
/// ## Optical Ports
///   - Inputs
///     - `input`
///   - Outputs
///     - `reflected`
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `curvature`
///   - `clear aperture`: the extent of the mirror, measured perpendicular to the axis of the
///     parent parabola around the point the chief ray hits; a ray beyond it misses the mirror
pub struct ParabolicMirror {
    node_attr: NodeAttr,
}
impl Default for ParabolicMirror {
    /// Create a parabolic mirror with a focal length of 1 meter.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("parabolic mirror");
        node_attr
            .create_property_with_validator(
                "focal length",
                "focal length",
                Validator::AndValidator {
                    validators: vec![Validator::NumericIsFinite, Validator::NumericIsNotZero],
                },
                // and_validator(vec![numeric_is_not_zero(), numeric_is_finite()]),
                meter!(1.0).into(),
            )
            .unwrap();
        node_attr
            .create_property_with_validator(
                "off-axis angle",
                "off axis angle",
                Validator::AndValidator {
                    validators: vec![
                        Validator::AngleInRange {
                            min: degree!(-180.),
                            max: degree!(180.),
                            inclusive: false,
                        },
                        Validator::NumericIsFinite,
                    ],
                },
                degree!(0.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "collimating",
                "collimation flag. True if the parabola should collimate, false otherwise",
                false.into(),
            )
            .unwrap();

        node_attr
            .create_property(
                "off-axis direction",
                "off axis direction in the local coordinate system",
                Vector2::new(1., 0.).into(),
            )
            .unwrap();
        create_surface_properties(&mut node_attr).unwrap();

        let mut parabola = Self { node_attr };
        parabola.update_surfaces().unwrap();

        parabola
            .set_coating(
                &PortType::Input,
                "input_1",
                &CoatingConstantR::new(percent!(100.0)).unwrap().into(),
            )
            .unwrap();

        parabola
            .set_coating(
                &PortType::Output,
                "output_1",
                &CoatingConstantR::new(percent!(100.0)).unwrap().into(),
            )
            .unwrap();
        parabola
    }
}
impl ParabolicMirror {
    /// Creates a new on-axis [`ParabolicMirror`] node.
    ///
    /// This function creates an infinitely thin, on-axis (0°) parabolic mirror with a given focal length.
    /// # Attributes
    /// - `name`: name of the node
    /// - `focal_length`: focal length of the parabolic mirror
    /// - `collimating`: flag that defines if the parabola should collimate a beam (true) or should focus the beam (false)
    ///
    /// # Errors
    ///
    /// This function returns an error if the given focal length is zero or not finite.
    pub fn new(name: &str, focal_length: Length, collimating: bool) -> OpmResult<Self> {
        let mut parabola = Self::default();
        parabola.node_attr.set_name(name);
        parabola
            .node_attr
            .set_property("focal length", focal_length.into())?;
        parabola
            .node_attr
            .set_property("collimating", collimating.into())?;
        parabola.update_surfaces()?;
        Ok(parabola)
    }

    /// Creates a new x off-axis [`ParabolicMirror`] node.
    ///
    /// This function creates an infinitely thin, off-axis parabolic mirror with a given focal length and reflection within the x-z plane.
    /// # Attributes
    /// - `name`: name of the node
    /// - `focal_length`: focal length of the parabolic mirror
    /// - `collimating`: flag that defines if the parabola should collimate a beam (true) or should focus the beam (false)
    /// - `oa_angle`: off-axis angle of the parabolic mirror
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// - the focal length is zero or not finite.
    /// - the off-axis angle is not finite
    pub fn new_with_off_axis_x(
        name: &str,
        focal_length: Length,
        collimating: bool,
        oa_angle: Angle,
    ) -> OpmResult<Self> {
        let mut parabola = Self::default();
        parabola.node_attr.set_name(name);
        parabola
            .node_attr
            .set_property("focal length", focal_length.into())?;
        parabola
            .node_attr
            .set_property("collimating", collimating.into())?;
        parabola
            .node_attr
            .set_property("off-axis angle", oa_angle.into())?;
        parabola
            .node_attr
            .set_property("off-axis direction", Vector2::new(1., 0.).into())?;
        parabola.update_surfaces()?;
        Ok(parabola)
    }
    /// Creates a new y off-axis [`ParabolicMirror`] node.
    ///
    /// This function creates an infinitely thin, off-axis parabolic mirror with a given focal length and reflection within the y-z plane.
    /// # Attributes
    /// - `name`: name of the node
    /// - `focal_length`: focal length of the parabolic mirror
    /// - `collimating`: flag that defines if the parabola should collimate a beam (true) or should focus the beam (false)
    /// - `oa_angle`: off-axis angle of the parabolic mirror
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// - the focal length is zero or not finite.
    /// - the off-axis angle is not finite
    pub fn new_with_off_axis_y(
        name: &str,
        focal_length: Length,
        collimating: bool,
        oa_angle: Angle,
    ) -> OpmResult<Self> {
        let mut parabola = Self::default();
        parabola.node_attr.set_name(name);
        parabola
            .node_attr
            .set_property("focal length", focal_length.into())?;
        parabola
            .node_attr
            .set_property("collimating", collimating.into())?;
        parabola
            .node_attr
            .set_property("off-axis angle", oa_angle.into())?;
        parabola
            .node_attr
            .set_property("off-axis direction", Vector2::new(0., 1.).into())?;
        parabola.update_surfaces()?;
        Ok(parabola)
    }
    /// Creates a new off-axis [`ParabolicMirror`] node.
    ///
    /// This function creates an infinitely thin, off-axis parabolic mirror with a given focal length and reflection with a defined direction
    /// # Attributes
    /// - `name`: name of the node
    /// - `focal_length`: focal length of the parabolic mirror
    /// - `collimating`: flag that defines if the parabola should collimate a beam (true) or should focus the beam (false)
    /// - `oa_angle`: off-axis angle of the parabolic mirror
    /// - `oa_dir`: projected direction of the reflected ray in the x-y plane of the parabola
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// - the focal length is zero or not finite.
    /// - the off-axis angle is not finite
    /// - the off-axis direction is not finite or if its norm is zero
    pub fn new_with_off_axis(
        name: &str,
        focal_length: Length,
        collimating: bool,
        oa_angle: Angle,
        oa_dir: Vector2<f64>,
    ) -> OpmResult<Self> {
        let mut parabola = Self::default();
        parabola.node_attr.set_name(name);
        parabola
            .node_attr
            .set_property("focal length", focal_length.into())?;
        parabola
            .node_attr
            .set_property("collimating", collimating.into())?;
        parabola
            .node_attr
            .set_property("off-axis angle", oa_angle.into())?;
        if !oa_dir.x.is_finite() || !oa_dir.y.is_finite() || oa_dir.norm() < f64::EPSILON {
            return Err(OpossumError::Other(
                "off-axis direction values must be finite and the vector norm non-zero".into(),
            ));
        }
        parabola
            .node_attr
            .set_property("off-axis direction", oa_dir.normalize().into())?;
        parabola.update_surfaces()?;
        Ok(parabola)
    }
    /// Return where the parent parabola lies relative to this node.
    ///
    /// The node sits where the chief ray hits the mirror.
    ///
    /// # Returns
    ///
    /// The frame of the parent axis through the node: a pure rotation of the node frame whose z
    /// axis is parallel to the axis of the parent parabola. And the vertex of the parent parabola,
    /// stated in that frame: a pure translation by the decenter and the sag of the hit point.
    ///
    /// # Errors
    ///
    /// This function returns an error if the parabola's properties cannot be read.
    fn calc_off_axis_frames(&self) -> OpmResult<(Isometry, Isometry)> {
        let (focal_length, oa_angle, oa_dir, collimating) = self.get_parabola_attributes()?;
        let tan_val = (oa_angle / 2.).tan().value;
        let decenter_x = oa_angle.sin() * focal_length;
        let z_shift = focal_length * (1. / tan_val.mul_add(tan_val, 1.) - oa_angle.cos().value);
        let z_rot_angle = f64::atan2(oa_dir.y, oa_dir.x);

        let vertex = Isometry::new_translation(Point3::new(decenter_x, meter!(0.), z_shift))?;
        let mut axis = Isometry::new_rotation(radian!(0., 0., z_rot_angle))?;
        if collimating {
            let normal_vector = vector![
                decenter_x.value,
                0.,
                -2. * self.calc_parent_focal_length()?.value
            ];
            let trans_normal_vector = axis.transform_vector_f64(&normal_vector);
            let rot = Isometry::new_from_transform(Isometry3::new(
                Vector3::zeros(),
                trans_normal_vector.normalize() * std::f64::consts::PI,
            ));
            axis = rot.append(&axis);
        }
        Ok((axis, vertex))
    }
    fn calc_parent_focal_length(&self) -> OpmResult<Length> {
        let Ok(Proptype::Length(focal_length)) = self.node_attr.get_property("focal length") else {
            return Err(OpossumError::Analysis("cannot read focal length".into()));
        };
        let Ok(Proptype::Angle(oa_angle)) = self.node_attr.get_property("off-axis angle") else {
            return Err(OpossumError::Analysis("cannot read off-axis angle".into()));
        };
        let tan_val = (*oa_angle / 2.).tan().value;
        Ok(*focal_length / tan_val.mul_add(tan_val, 1.))
    }
    /// Returns / modifies a [`ParabolicMirror`] with a given off-axis angle.
    ///
    /// The angle defines the off axis angle between the focal direction of the mother parabola, hit at its center, and the off-axis focal direction.
    /// The give angle denotes the full angle between an incoming and a reflected beam. Effectively this introduces a decentering
    /// of the node during positioning in 3D space such that the desired angle is met.
    ///
    /// # Errors
    ///
    /// This function will return an error if the node properties cannot be set.
    pub fn with_oap_angle(mut self, oa_angle: Angle) -> OpmResult<Self> {
        self.set_property("off-axis angle", oa_angle.into())?;
        self.update_surfaces()?;
        Ok(self)
    }
    /// Returns / modifies a [`ParabolicMirror`] with a given off-axis direction.
    ///
    /// The off-axis direction defines the projected direction of the reflected beam.
    /// E.g. if the parabola reflects the beam with an off-axis angle of 45° in x direction, the respective vector would just be (1,0).
    ///
    /// # Errors
    ///
    /// This function will return an error if the node properties cannot be set.
    pub fn with_oap_direction(mut self, oa_dir: Vector2<f64>) -> OpmResult<Self> {
        self.set_property("off-axis direction", oa_dir.normalize().into())?;
        self.update_surfaces()?;
        Ok(self)
    }
    /// Returns the parabola-specific node attributed
    ///
    /// This function returns a tuple containing:
    /// - 0: the focal length
    /// - 1: the off-axis-angle
    /// - 2: the off-axis direction
    /// - 3: the collimating flag
    ///
    /// # Errors
    /// This function errors if one of the attributes cannot be read from the properties.
    pub fn get_parabola_attributes(&self) -> OpmResult<(Length, Angle, Vector2<f64>, bool)> {
        let Ok(Proptype::Length(focal_length)) = self.node_attr.get_property("focal length") else {
            return Err(OpossumError::Analysis("cannot read focal length".into()));
        };
        let Ok(Proptype::Angle(oa_angle)) = self.node_attr.get_property("off-axis angle") else {
            return Err(OpossumError::Analysis("cannot read off-axis angle".into()));
        };
        let Ok(Proptype::Vec2(oa_dir)) = self.node_attr.get_property("off-axis direction") else {
            return Err(OpossumError::Analysis(
                "cannot read off-axis direction".into(),
            ));
        };
        let Ok(Proptype::Bool(collimating)) = self.node_attr.get_property("collimating") else {
            return Err(OpossumError::Analysis(
                "cannot read collimation flag".into(),
            ));
        };

        Ok((*focal_length, *oa_angle, *oa_dir, *collimating))
    }
}

impl OpticNode for ParabolicMirror {
    fn geometry(&self) -> OpmResult<Option<Geometry>> {
        let (axis, vertex) = self.calc_off_axis_frames()?;
        Ok(Some(Geometry::surface(
            axis,
            SurfaceShape::Parabola {
                focal_length: -1. * self.calc_parent_focal_length()?,
            },
            vertex,
            Some(self.clear_aperture()?),
        )?))
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.install_geometry(&["input_1"], &["output_1"])
    }
}
impl AnalysisGhostFocus for ParabolicMirror {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];

        let mut rays_bundle = incoming_data
            .get(in_port)
            .map_or_else(Vec::<Rays>::new, std::clone::Clone::clone);
        let mut ray_trace_config = RayTraceConfig::default();
        ray_trace_config.set_missed_surface_strategy(MissedSurfaceStrategy::Ignore);
        for rays in &mut rays_bundle {
            let mut input = LightResult::default();
            input.insert(in_port.clone(), LightData::Geometric(rays.clone()));
            let out = AnalysisRayTrace::analyze(self, input, &ray_trace_config)?;

            if let Some(LightData::Geometric(r)) = out.get(out_port) {
                *rays = r.clone();
            }
        }

        let Some(surf) = self.get_optic_surface_mut(in_port) else {
            return Err(OpossumError::Analysis(format!(
                "Cannot find surface: \"{in_port}\" of node: \"{}\"",
                self.node_attr().name()
            )));
        };
        for rays in &mut rays_bundle {
            surf.evaluate_fluence_of_ray_bundle(rays, config.fluence_estimator())?;
        }

        let mut out_light_rays = LightRays::default();
        out_light_rays.insert(out_port.clone(), rays_bundle.clone());
        Ok(out_light_rays)
    }
}
impl AnalysisEnergy for ParabolicMirror {}
impl AnalysisRayTrace for ParabolicMirror {
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
        let LightData::Geometric(mut rays) = data.clone() else {
            return Err(OpossumError::Analysis(
                "expected ray data at input port".into(),
            ));
        };
        if rays.is_empty() {
            return Ok(LightResult::default());
        }
        let Some(surf) = self.get_optic_surface_mut(in_port) else {
            return Err(OpossumError::Analysis("no surface found. Aborting".into()));
        };

        let refraction_intended = false;
        let (mut reflected_rays, _) = rays.refract_on_surface(
            surf,
            None,
            refraction_intended,
            config.missed_surface_strategy(),
        )?;
        match self.ports().aperture(&PortType::Input, in_port) {
            // Only the components' rims decide where the optical axis runs.
            Some(_) if config.is_positioning_run() => {}
            Some(aperture) => {
                reflected_rays.apodize(aperture, &self.effective_surface_iso(in_port)?)?;
                reflected_rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
            _ => {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            }
        }
        let light_data = LightData::Geometric(reflected_rays);
        let light_result = LightResult::from([(out_port.into(), light_data)]);
        Ok(light_result)
    }

    fn calc_node_positions(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        AnalysisRayTrace::analyze(self, incoming_data, config)
    }
}

#[cfg(test)]
mod test {
    use std::f64::consts::FRAC_1_SQRT_2;

    use crate::{
        analyzers::{
            GhostFocusConfig, RayTraceConfig,
            energy::{AnalysisEnergy, EnergyConfig},
            ghostfocus::AnalysisGhostFocus,
            raytrace::AnalysisRayTrace,
        },
        apertures::{ApertureShape, CircleShape},
        core_optics::{NodeAttrExt, OpticNode, node_attr::NodePositioning},
        degree,
        distributions::position::Hexapolar,
        error::OpmResult,
        geometry::body::CLEAR_APERTURE,
        joule,
        light::{
            LightData, LightResult, Ray, Rays, light_result::light_result_to_light_rays,
            spectrum_helper::create_he_ne_spec,
        },
        meter, millimeter, nanometer,
        nodes::{
            ParabolicMirror,
            test_helper::helper::{
                test_clear_aperture_absent_in_file,
                test_reflections_beyond_clear_aperture_are_lost, test_reflects_completely,
            },
        },
        properties::Proptype,
        utils::geom_transformation::Isometry,
    };
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use nalgebra::{Matrix4, Vector2, Vector3};
    #[test]
    fn default() -> OpmResult<()> {
        let parabola = ParabolicMirror::default();
        assert_eq!(parabola.node_attr.name(), "parabolic mirror");

        let Proptype::Length(focal_length) = parabola.node_attr.get_property("focal length")?
        else {
            panic!()
        };
        assert_relative_eq!(focal_length.value, 1.);
        let Proptype::Bool(collimate) = parabola.node_attr.get_property("collimating")? else {
            panic!()
        };
        assert!(!collimate);

        let Proptype::Angle(angle) = parabola.node_attr.get_property("off-axis angle")? else {
            panic!()
        };
        assert_relative_eq!(angle.value, 0.);
        let Proptype::Vec2(dir) = parabola.node_attr.get_property("off-axis direction")? else {
            panic!()
        };
        assert_relative_eq!(*dir, Vector2::new(1., 0.));
        Ok(())
    }
    #[test]
    fn reflects_completely() {
        test_reflects_completely::<ParabolicMirror>();
    }
    #[test]
    fn clear_aperture_absent_in_file() -> OpmResult<()> {
        test_clear_aperture_absent_in_file::<ParabolicMirror>()
    }
    #[test]
    fn reflections_beyond_clear_aperture_are_lost() -> OpmResult<()> {
        test_reflections_beyond_clear_aperture_are_lost::<ParabolicMirror>()
    }
    /// A collimating 90° off-axis parabola of 50 mm diameter, as a supplier quotes it: the circle is
    /// measured perpendicular to the parent axis, around the point the chief ray hits. A ray
    /// parallel to the parent axis therefore hits exactly when it passes within 25 mm of the chief
    /// ray's reflection, however far the curved surface bends away in between.
    #[test]
    fn an_off_axis_parabola_measures_its_clear_aperture_along_the_parent_axis() -> OpmResult<()> {
        let mut parabola =
            ParabolicMirror::new_with_off_axis_x("oap", millimeter!(100.0), true, degree!(90.0))?;
        parabola.set_property(
            CLEAR_APERTURE,
            ApertureShape::BinaryCircle(CircleShape::new(millimeter!(25.0))?).into(),
        )?;
        parabola.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let mut reflect = |ray: Ray| -> OpmResult<Option<Ray>> {
            let input =
                LightResult::from([("input_1".into(), LightData::Geometric(Rays::from(ray)))]);
            let output =
                AnalysisRayTrace::analyze(&mut parabola, input, &RayTraceConfig::default())?;
            let Some(LightData::Geometric(reflected)) = output.get("output_1") else {
                panic!("expected ray data at the output port");
            };
            Ok(reflected.iter().find(|ray| ray.valid()).cloned())
        };
        // The chief ray comes from the focus along z and leaves parallel to the parent axis.
        let chief = reflect(Ray::new_collimated(
            millimeter!(0.0, 0.0, -10.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?)?
        .expect("the chief ray hits the mirror");
        let parent_axis = chief.direction().normalize();
        let across = Vector3::y();
        let within = parent_axis.cross(&across);
        for direction in [across, -across, within, -within] {
            for (offset, hits) in [(24.9, true), (25.1, false)] {
                // Sent back along the reflected beam, from 100 mm out.
                let start = direction * offset + parent_axis * 100.0;
                let ray = Ray::new(
                    millimeter!(start.x, start.y, start.z),
                    -parent_axis,
                    nanometer!(1000.0),
                    joule!(1.0),
                )?;
                assert_eq!(
                    reflect(ray)?.is_some(),
                    hits,
                    "{offset} mm off the chief ray towards {direction:?}"
                );
            }
        }
        Ok(())
    }
    #[test]
    fn new() {
        assert!(ParabolicMirror::new("Parabola", meter!(1.), true).is_ok());
        assert!(ParabolicMirror::new("Parabola", meter!(-1.), true).is_ok());
        assert!(ParabolicMirror::new("Parabola", meter!(-1.), false).is_ok());
        assert!(ParabolicMirror::new("Parabola", meter!(1.), false).is_ok());

        assert!(ParabolicMirror::new("Parabola", meter!(0.), false).is_err());
        assert!(ParabolicMirror::new("Parabola", meter!(f64::NAN), false).is_err());
        assert!(ParabolicMirror::new("Parabola", meter!(f64::INFINITY), false).is_err());
        assert!(ParabolicMirror::new("Parabola", meter!(f64::NEG_INFINITY), false).is_err());
    }
    #[test]
    fn name() -> OpmResult<()> {
        let p = ParabolicMirror::new("Parabola", meter!(1.), true)?;
        assert_eq!(p.node_attr.name(), "Parabola");
        Ok(())
    }
    #[test]
    fn new_with_off_axis_x() {
        assert!(
            ParabolicMirror::new_with_off_axis_x("Parabola", meter!(1.), true, degree!(45.))
                .is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_x("Parabola", meter!(1.), true, degree!(-45.))
                .is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_x("Parabola", meter!(1.), true, degree!(0.)).is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_x("Parabola", meter!(1.), true, degree!(180.))
                .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_x("Parabola", meter!(1.), true, degree!(190.))
                .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_x("Parabola", meter!(1.), true, degree!(-180.))
                .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_x("Parabola", meter!(1.), true, degree!(-190.))
                .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_x("Parabola", meter!(1.), true, degree!(f64::NAN))
                .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_x(
                "Parabola",
                meter!(1.),
                true,
                degree!(f64::INFINITY)
            )
            .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_x(
                "Parabola",
                meter!(1.),
                true,
                degree!(f64::NEG_INFINITY)
            )
            .is_err()
        );
    }
    #[test]
    fn new_with_off_axis_y() {
        assert!(
            ParabolicMirror::new_with_off_axis_y("Parabola", meter!(1.), true, degree!(45.))
                .is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_y("Parabola", meter!(1.), true, degree!(-45.))
                .is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_y("Parabola", meter!(1.), true, degree!(0.)).is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_y("Parabola", meter!(1.), true, degree!(180.))
                .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_y("Parabola", meter!(1.), true, degree!(190.))
                .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_y("Parabola", meter!(1.), true, degree!(-180.))
                .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_y("Parabola", meter!(1.), true, degree!(-190.))
                .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_y("Parabola", meter!(1.), true, degree!(f64::NAN))
                .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_y(
                "Parabola",
                meter!(1.),
                true,
                degree!(f64::INFINITY)
            )
            .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis_y(
                "Parabola",
                meter!(1.),
                true,
                degree!(f64::NEG_INFINITY)
            )
            .is_err()
        );
    }
    #[test]
    fn new_with_off_axis() {
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(1., 0.)
            )
            .is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(-1., 0.)
            )
            .is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(0., 1.)
            )
            .is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(0., -1.)
            )
            .is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(-1., -1.)
            )
            .is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(1., -1.)
            )
            .is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(1., 1.)
            )
            .is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(-1., 1.)
            )
            .is_ok()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(0., 0.)
            )
            .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(f64::NAN, 0.)
            )
            .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(f64::INFINITY, 0.)
            )
            .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(f64::NEG_INFINITY, 0.)
            )
            .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(0., f64::NAN)
            )
            .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(0., f64::INFINITY)
            )
            .is_err()
        );
        assert!(
            ParabolicMirror::new_with_off_axis(
                "Parabola",
                meter!(1.),
                true,
                degree!(45.),
                Vector2::new(0., f64::NEG_INFINITY)
            )
            .is_err()
        );
    }
    #[test]
    fn calc_off_axis_frames() -> OpmResult<()> {
        let parabola = ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(0., 1.),
        )?;
        let transform_mat = Matrix4::from_vec(vec![
            -0.,
            1.,
            -0.,
            -0.,
            -FRAC_1_SQRT_2,
            -0.,
            -FRAC_1_SQRT_2,
            -0.6035533905932736,
            -FRAC_1_SQRT_2,
            0.,
            FRAC_1_SQRT_2,
            -0.3964466094067264,
            0.,
            0.,
            0.,
            1.,
        ])
        .transpose();
        let (axis, vertex) = parabola.calc_off_axis_frames()?;
        assert_relative_eq!(
            transform_mat,
            axis.append(&vertex).get_transform().to_matrix(),
            epsilon = 3. * f64::EPSILON
        );
        // The parent axis passes through the node; the vertex is offset without a turn.
        assert_eq!(axis.translation(), millimeter!(0.0, 0.0, 0.0));
        assert_eq!(vertex.rotation(), degree!(0.0, 0.0, 0.0));
        Ok(())
    }
    /// The off-axis frame is parallel to the axis of the parent parabola: a collimating mirror
    /// sends the chief ray from its focus along that axis, a focusing mirror receives it along it.
    #[test]
    fn the_off_axis_frame_is_parallel_to_the_parent_axis() -> OpmResult<()> {
        for collimating in [true, false] {
            let mut parabola = ParabolicMirror::new_with_off_axis_y(
                "oap",
                millimeter!(100.0),
                collimating,
                degree!(90.0),
            )?;
            parabola.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
            let chief_ray = Ray::new_collimated(
                millimeter!(0.0, 0.0, -10.0),
                nanometer!(1000.0),
                joule!(1.0),
            )?;
            let input = LightResult::from([(
                "input_1".into(),
                LightData::Geometric(Rays::from(chief_ray)),
            )]);
            let output =
                AnalysisRayTrace::analyze(&mut parabola, input, &RayTraceConfig::default())?;
            let Some(LightData::Geometric(reflected)) = output.get("output_1") else {
                panic!("the mirror reflects the chief ray");
            };
            let along_parent_axis = if collimating {
                reflected
                    .iter()
                    .next()
                    .expect("one reflected ray")
                    .direction()
            } else {
                Vector3::z()
            };
            let (axis, _) = parabola.calc_off_axis_frames()?;
            let frame_z = axis.transform_vector_f64(&Vector3::z());
            assert_abs_diff_eq!(
                frame_z.cross(&along_parent_axis.normalize()).norm(),
                0.0,
                epsilon = 1e-12
            );
        }
        Ok(())
    }

    #[test]
    fn calc_parent_focal_length() -> OpmResult<()> {
        let parabola = ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(90.),
            Vector2::new(0., 1.),
        )?;
        assert_relative_eq!(parabola.calc_parent_focal_length()?.value, 0.5);
        Ok(())
    }

    #[test]
    fn with_oap_angle() -> OpmResult<()> {
        let parabola = ParabolicMirror::default().with_oap_angle(degree!(45.))?;
        let Proptype::Angle(angle) = parabola.node_attr.get_property("off-axis angle")? else {
            panic!()
        };
        assert_relative_eq!(angle.value, 45. / 180. * std::f64::consts::PI);
        Ok(())
    }
    #[test]
    fn with_oap_direction() -> OpmResult<()> {
        let parabola = ParabolicMirror::default().with_oap_direction(Vector2::new(0.35, 8.35))?;
        let Proptype::Vec2(oa_dir) = parabola.node_attr.get_property("off-axis direction")? else {
            panic!()
        };
        assert_relative_eq!(*oa_dir, Vector2::new(0.35, 8.35).normalize());
        Ok(())
    }
    #[test]
    fn analysis_raytrace_empty_input() -> OpmResult<()> {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Geometric(Rays::default());
        let input = LightResult::from([("input_1".into(), light_data)]);
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }

    #[test]
    fn analysis_raytrace_no_input() -> OpmResult<()> {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Fourier;
        let input = LightResult::from([("output_1".into(), light_data)]);
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }

    #[test]
    fn analysis_raytrace_lightdata_energy() -> OpmResult<()> {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Energy(create_he_ne_spec(1.0)?);
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).is_err());
        Ok(())
    }
    #[test]
    fn analysis_raytrace_lightdata_ghost_focus() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::GhostFocus(vec![Rays::default()]);
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).is_err());
    }

    #[test]
    fn analysis_raytrace_lightdata_fourier() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Fourier;
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).is_err());
    }

    #[test]
    fn analysis_raytrace_no_iso() -> OpmResult<()> {
        let mut node = ParabolicMirror::default();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Hexapolar::new(millimeter!(1.), 3)?,
        )?;
        let light_data = LightData::Geometric(rays);
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).is_err());
        Ok(())
    }
    #[test]
    fn analysis_raytrace() -> OpmResult<()> {
        let mut node = ParabolicMirror::default();
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Hexapolar::new(millimeter!(1.), 3)?,
        )?;
        let light_data = LightData::Geometric(rays);
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).is_ok());
        Ok(())
    }

    #[test]
    fn analysis_energy() -> OpmResult<()> {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Energy(create_he_ne_spec(1.0)?);
        let input = LightResult::from([("input_1".into(), light_data)]);
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default());
        assert!(output.is_ok());
        assert!(!output?.is_empty());
        Ok(())
    }

    #[test]
    fn analysis_energy_no_input() -> OpmResult<()> {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Fourier;
        let input = LightResult::from([("output_1".into(), light_data)]);
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }

    #[test]
    fn analysis_energy_lightdata_raytrace() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Geometric(Rays::default());
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default()).is_ok());
    }
    #[test]
    fn analysis_energy_lightdata_ghost_focus() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::GhostFocus(vec![Rays::default()]);
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default()).is_ok());
    }
    #[test]
    fn analysis_energy_lightdata_fourier() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Fourier;
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default()).is_ok());
    }
    #[test]
    fn analysis_ghost_focus_empty_input() -> OpmResult<()> {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::GhostFocus(Vec::<Rays>::new());
        let input =
            light_result_to_light_rays(LightResult::from([("input_1".into(), light_data)]))?;
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            input,
            &GhostFocusConfig::default(),
            &mut Vec::<Rays>::new(),
            0,
        )?;
        assert!(output.values().last().unwrap().is_empty());
        Ok(())
    }
    #[test]
    fn analysis_ghost_focus_no_input() -> OpmResult<()> {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::GhostFocus(vec![Rays::default()]);
        let input =
            light_result_to_light_rays(LightResult::from([("output_1".into(), light_data)]))?;
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            input,
            &GhostFocusConfig::default(),
            &mut Vec::<Rays>::new(),
            0,
        )?;
        assert!(output.values().last().unwrap().is_empty());
        Ok(())
    }
    #[test]
    fn analysis_ghost_focus_no_iso() -> OpmResult<()> {
        let mut node = ParabolicMirror::default();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Hexapolar::new(millimeter!(1.), 3)?,
        )?;
        let light_data = LightData::GhostFocus(vec![rays]);
        let input =
            light_result_to_light_rays(LightResult::from([("input_1".into(), light_data)]))?;
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            input,
            &GhostFocusConfig::default(),
            &mut Vec::<Rays>::new(),
            0,
        );
        assert!(output.is_err());
        Ok(())
    }
    #[test]
    fn analysis_ghost_focus() -> OpmResult<()> {
        let mut node = ParabolicMirror::default();
        node.set_positioning(NodePositioning::Absolute(Isometry::new_along_z(
            millimeter!(10.0),
        )?))?;
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Hexapolar::new(millimeter!(1.), 3)?,
        )?;
        let light_data = LightData::GhostFocus(vec![rays]);
        let input =
            light_result_to_light_rays(LightResult::from([("input_1".into(), light_data)]))?;
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            input,
            &GhostFocusConfig::default(),
            &mut Vec::<Rays>::new(),
            0,
        );
        assert!(output.is_ok());
        Ok(())
    }
    #[test]
    fn calc_node_position() -> OpmResult<()> {
        let mut node = ParabolicMirror::default();
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Hexapolar::new(millimeter!(1.), 3)?,
        )?;
        let light_data = LightData::Geometric(rays);
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(
            node.calc_node_positions(input, &RayTraceConfig::default())
                .is_ok()
        );
        Ok(())
    }
}
