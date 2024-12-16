use core::f64;

use std::{cell::RefCell, rc::Rc};

use nalgebra::{vector, Isometry3, Point3, Vector2, Vector3};
use uom::si::f64::{Angle, Length};

use crate::{
    analyzers::{
        energy::AnalysisEnergy,
        ghostfocus::AnalysisGhostFocus,
        raytrace::{AnalysisRayTrace, MissedSurfaceStrategy},
        Analyzable, GhostFocusConfig, RayTraceConfig,
    },
    coatings::CoatingType,
    degree,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    light_result::{LightRays, LightResult},
    lightdata::LightData,
    meter,
    optic_node::{Alignable, OpticNode, LIDT},
    optic_ports::PortType,
    properties::Proptype,
    radian,
    rays::Rays,
    surface::{geo_surface::GeoSurfaceRef, optic_surface::OpticSurface, Parabola},
    utils::geom_transformation::Isometry,
};

use super::NodeAttr;

#[derive(Debug, Clone)]
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
pub struct ParabolicMirror {
    node_attr: NodeAttr,
}
impl Default for ParabolicMirror {
    /// Create a parabolic mirror with a focal length of 1 meter.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("parabolic mirror");
        node_attr
            .create_property("focal length", "focal length", meter!(1.0).into())
            .unwrap();
        node_attr
            .create_property("oa angle", "off axis angle", degree!(0.0).into())
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
                "oa direction",
                "off axis direction in the local coordinate system",
                Vector2::new(1., 0.).into(),
            )
            .unwrap();

        let mut parabola = Self { node_attr };
        parabola.update_surfaces().unwrap();

        parabola
            .ports_mut()
            .set_coating(
                &PortType::Input,
                "input_1",
                &CoatingType::ConstantR { reflectivity: 1.0 },
            )
            .unwrap();

        parabola
            .ports_mut()
            .set_coating(
                &PortType::Output,
                "output_1",
                &CoatingType::ConstantR { reflectivity: 1.0 },
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
        if !focal_length.is_normal() {
            return Err(OpossumError::Other(
                "focal length must not be 0.0 and finite".into(),
            ));
        }
        let mut parabola = Self::default();
        parabola.node_attr.set_name(name);
        parabola.set_parabola_properties(focal_length, collimating, None, None)?;
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
        Self::check_attributes(focal_length, Some(&oa_angle), None)?;

        let mut parabola = Self::default();
        parabola.node_attr.set_name(name);
        parabola.set_parabola_properties(
            focal_length,
            collimating,
            Some(&oa_angle),
            Some(&Vector2::new(1., 0.)),
        )?;
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
        Self::check_attributes(focal_length, Some(&oa_angle), None)?;

        let mut parabola = Self::default();
        parabola.node_attr.set_name(name);
        parabola.set_parabola_properties(
            focal_length,
            collimating,
            Some(&oa_angle),
            Some(&Vector2::new(0., 1.)),
        )?;

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
        Self::check_attributes(focal_length, Some(&oa_angle), Some(&oa_dir))?;

        let mut parabola = Self::default();
        parabola.node_attr.set_name(name);
        parabola.set_parabola_properties(
            focal_length,
            collimating,
            Some(&oa_angle),
            Some(&oa_dir),
        )?;

        parabola.update_surfaces()?;
        Ok(parabola)
    }

    /// checks the validity of the provided node attributes of thie parabola
    fn check_attributes(
        focal_length: Length,
        oa_angle_opt: Option<&Angle>,
        oa_dir_opt: Option<&Vector2<f64>>,
    ) -> OpmResult<()> {
        if !focal_length.is_normal() {
            return Err(OpossumError::Other(
                "focal length must not be 0.0 and finite".into(),
            ));
        };
        if let Some(oa_angle) = oa_angle_opt {
            if !oa_angle.is_finite() {
                return Err(OpossumError::Other("off-axis angle and finite".into()));
            }
            if oa_angle.value.abs() >= f64::consts::PI {
                return Err(OpossumError::Other(
                    "off-axis angle must be smaller than 180°".into(),
                ));
            }
        };
        if let Some(oa_dir) = oa_dir_opt {
            if !oa_dir.x.is_finite() || !oa_dir.y.is_finite() || oa_dir.norm() < f64::EPSILON {
                return Err(OpossumError::Other(
                    "off-axis direction values must be finite and the vector norm non-zero".into(),
                ));
            }
        };
        Ok(())
    }

    /// sets the properties of this parabola
    fn set_parabola_properties(
        &mut self,
        focal_length: Length,
        collimating: bool,
        oa_angle_opt: Option<&Angle>,
        oa_dir_opt: Option<&Vector2<f64>>,
    ) -> OpmResult<()> {
        Self::check_attributes(focal_length, oa_angle_opt, oa_dir_opt)?;

        self.node_attr
            .set_property("focal length", focal_length.into())?;
        self.node_attr
            .set_property("collimating", collimating.into())?;

        if let Some(oa_angle) = oa_angle_opt {
            self.node_attr
                .set_property("oa angle", (*oa_angle).into())?;
        }

        if let Some(oa_dir) = oa_dir_opt {
            self.node_attr
                .set_property("oa direction", oa_dir.normalize().into())?;
        }
        Ok(())
    }

    fn calc_off_axis_isometry(&self) -> OpmResult<Isometry> {
        let (focal_length, oa_angle, oa_dir, collimating) = self.get_parabola_attributes()?;
        let tan_val = (oa_angle / 2.).tan().value;
        let decenter_x = oa_angle.sin() * focal_length;
        let z_shift = focal_length * (1. / tan_val.mul_add(tan_val, 1.) - oa_angle.cos().value);
        let z_rot_angle = f64::atan2(oa_dir.y, oa_dir.x);

        let iso = Isometry::new_translation(Point3::new(decenter_x, meter!(0.), z_shift))?;
        let rot_iso = Isometry::new_rotation(radian!(0., 0., z_rot_angle))?;
        let mut tot_iso = rot_iso.append(&iso);
        if collimating {
            let normal_vector = vector![
                decenter_x.value,
                0.,
                -2. * self.calc_parent_focal_length()?.value
            ];
            let trans_normal_vector = tot_iso.transform_vector_f64(&normal_vector);
            let rot = Isometry::new_from_transform(Isometry3::new(
                Vector3::zeros(),
                trans_normal_vector.normalize() * std::f64::consts::PI,
            ));
            tot_iso = rot.append(&tot_iso);
        }
        Ok(tot_iso)
    }

    fn calc_parent_focal_length(&self) -> OpmResult<Length> {
        let Ok(Proptype::Length(focal_length)) = self.node_attr.get_property("focal length") else {
            return Err(OpossumError::Analysis("cannot read focal length".into()));
        };
        let Ok(Proptype::Angle(oa_angle)) = self.node_attr.get_property("oa angle") else {
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
        self.set_property("oa angle", oa_angle.into())?;
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
        self.set_property("oa direction", oa_dir.normalize().into())?;
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
        let Ok(Proptype::Angle(oa_angle)) = self.node_attr.get_property("oa angle") else {
            return Err(OpossumError::Analysis("cannot read off-axis angle".into()));
        };
        let Ok(Proptype::Vec2(oa_dir)) = self.node_attr.get_property("oa direction") else {
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
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        let node_iso = self.effective_node_iso().unwrap_or_else(Isometry::identity);
        let anchor_point_iso = self.calc_off_axis_isometry()?;
        let total_iso = node_iso.append(&anchor_point_iso);
        let parabola = Parabola::new(-1. * self.calc_parent_focal_length()?, &total_iso)?;
        let para_geo_surface = GeoSurfaceRef(Rc::new(RefCell::new(parabola)));
        if let Some(optic_surf) = self
            .ports_mut()
            .get_optic_surface_mut(&"input_1".to_string())
        {
            optic_surf.set_geo_surface(para_geo_surface.clone());
            optic_surf.set_anchor_point_iso(anchor_point_iso.clone());
        } else {
            let mut optic_surf = OpticSurface::default();
            optic_surf.set_geo_surface(para_geo_surface.clone());
            optic_surf.set_anchor_point_iso(anchor_point_iso.clone());
            self.ports_mut()
                .add_optic_surface(&PortType::Input, "input_1", optic_surf)?;
        }
        if let Some(optic_surf) = self
            .ports_mut()
            .get_optic_surface_mut(&"output_1".to_string())
        {
            optic_surf.set_geo_surface(para_geo_surface);
            optic_surf.set_anchor_point_iso(anchor_point_iso);
        } else {
            let mut optic_surf = OpticSurface::default();
            optic_surf.set_geo_surface(para_geo_surface);
            optic_surf.set_anchor_point_iso(anchor_point_iso);
            self.ports_mut()
                .add_optic_surface(&PortType::Output, "output_1", optic_surf)?;
        }
        Ok(())
    }
}
impl Alignable for ParabolicMirror {}
impl Dottable for ParabolicMirror {
    fn node_color(&self) -> &str {
        "chocolate2"
    }
}
impl LIDT for ParabolicMirror {}
impl Analyzable for ParabolicMirror {
    // fn set_surface_iso(&mut self, port_str: &str, iso: &Isometry) -> OpmResult<()> {
    //     let parabola_iso = self.calc_and_set_off_axis_isometry()?;
    //     if let Some(input_surf) = self.get_optic_surface_mut(port_str) {
    //         input_surf.set_isometry(&iso.append(&parabola_iso));
    //     } else {
    //         return Err(OpossumError::OpticPort("No surface found.".into()));
    //     }
    //     Ok(())
    // }
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
        out_light_rays.insert(out_port.to_string(), rays_bundle.clone());
        Ok(out_light_rays)
    }
}
impl AnalysisEnergy for ParabolicMirror {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(data) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        Ok(LightResult::from([(out_port.into(), data.clone())]))
    }
}
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
        };

        let Some(surf) = self.get_optic_surface_mut(in_port) else {
            return Err(OpossumError::Analysis("no surface found. Aborting".into()));
        };

        let refraction_intended = false;
        let mut reflected_rays = rays.refract_on_surface(
            surf,
            None,
            refraction_intended,
            config.missed_surface_strategy(),
        )?;
        if let Some(aperture) = self.ports().aperture(&PortType::Input, in_port) {
            reflected_rays.apodize(aperture, &self.effective_surface_iso(in_port)?)?;
            reflected_rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
        } else {
            return Err(OpossumError::OpticPort("input aperture not found".into()));
        }
        let light_data = LightData::Geometric(reflected_rays);
        let light_result = LightResult::from([(out_port.into(), light_data)]);
        Ok(light_result)
    }

    fn calc_node_position(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        AnalysisRayTrace::analyze(self, incoming_data, config)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        analyzers::{
            energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
            GhostFocusConfig, RayTraceConfig,
        },
        degree, joule,
        light_result::{light_result_to_light_rays, LightResult},
        lightdata::{DataEnergy, LightData},
        meter, millimeter, nanometer,
        nodes::ParabolicMirror,
        optic_node::OpticNode,
        position_distributions::Hexapolar,
        properties::Proptype,
        rays::Rays,
        spectrum_helper::create_he_ne_spec,
        utils::geom_transformation::Isometry,
    };
    use approx::assert_relative_eq;
    use core::f64;
    use nalgebra::{Matrix4, Vector2};
    #[test]
    fn default() {
        let parabola = ParabolicMirror::default();
        assert_eq!(parabola.node_attr.name().as_str(), "parabolic mirror");

        let Proptype::Length(focal_length) =
            parabola.node_attr.get_property("focal length").unwrap()
        else {
            panic!()
        };
        assert_relative_eq!(focal_length.value, 1.);
        let Proptype::Bool(collimate) = parabola.node_attr.get_property("collimating").unwrap()
        else {
            panic!()
        };
        assert!(!collimate);

        let Proptype::Angle(angle) = parabola.node_attr.get_property("oa angle").unwrap() else {
            panic!()
        };
        assert_relative_eq!(angle.value, 0.);
        let Proptype::Vec2(dir) = parabola.node_attr.get_property("oa direction").unwrap() else {
            panic!()
        };
        assert_relative_eq!(*dir, Vector2::new(1., 0.));
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
    fn name() {
        let p = ParabolicMirror::new("Parabola", meter!(1.), true).unwrap();
        assert_eq!(p.node_attr.name().as_str(), "Parabola");
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
        assert!(ParabolicMirror::new_with_off_axis_x(
            "Parabola",
            meter!(1.),
            true,
            degree!(f64::NAN)
        )
        .is_err());
        assert!(ParabolicMirror::new_with_off_axis_x(
            "Parabola",
            meter!(1.),
            true,
            degree!(f64::INFINITY)
        )
        .is_err());
        assert!(ParabolicMirror::new_with_off_axis_x(
            "Parabola",
            meter!(1.),
            true,
            degree!(f64::NEG_INFINITY)
        )
        .is_err());
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
        assert!(ParabolicMirror::new_with_off_axis_y(
            "Parabola",
            meter!(1.),
            true,
            degree!(f64::NAN)
        )
        .is_err());
        assert!(ParabolicMirror::new_with_off_axis_y(
            "Parabola",
            meter!(1.),
            true,
            degree!(f64::INFINITY)
        )
        .is_err());
        assert!(ParabolicMirror::new_with_off_axis_y(
            "Parabola",
            meter!(1.),
            true,
            degree!(f64::NEG_INFINITY)
        )
        .is_err());
    }
    #[test]
    fn new_with_off_axis() {
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(1., 0.)
        )
        .is_ok());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(-1., 0.)
        )
        .is_ok());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(0., 1.)
        )
        .is_ok());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(0., -1.)
        )
        .is_ok());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(-1., -1.)
        )
        .is_ok());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(1., -1.)
        )
        .is_ok());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(1., 1.)
        )
        .is_ok());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(-1., 1.)
        )
        .is_ok());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(0., 0.)
        )
        .is_err());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(f64::NAN, 0.)
        )
        .is_err());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(f64::INFINITY, 0.)
        )
        .is_err());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(f64::NEG_INFINITY, 0.)
        )
        .is_err());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(0., f64::NAN)
        )
        .is_err());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(0., f64::INFINITY)
        )
        .is_err());
        assert!(ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(0., f64::NEG_INFINITY)
        )
        .is_err());
    }
    #[test]
    fn set_parabola_properties() {
        let mut parabola = ParabolicMirror::default();

        assert!(parabola
            .set_parabola_properties(meter!(10.), true, None, None)
            .is_ok());
        let Proptype::Length(focal_length) =
            parabola.node_attr.get_property("focal length").unwrap()
        else {
            panic!()
        };
        assert_relative_eq!(focal_length.value, 10.);
        let Proptype::Bool(collimate) = parabola.node_attr.get_property("collimating").unwrap()
        else {
            panic!()
        };
        assert!(collimate);

        let Proptype::Angle(angle) = parabola.node_attr.get_property("oa angle").unwrap() else {
            panic!()
        };
        assert_relative_eq!(angle.value, 0.);
        let Proptype::Vec2(dir) = parabola.node_attr.get_property("oa direction").unwrap() else {
            panic!()
        };
        assert_relative_eq!(*dir, Vector2::new(1., 0.));

        assert!(parabola
            .set_parabola_properties(
                meter!(10.),
                true,
                Some(&degree!(45.)),
                Some(&Vector2::new(3., 2.))
            )
            .is_ok());

        let Proptype::Angle(angle) = parabola.node_attr.get_property("oa angle").unwrap() else {
            panic!()
        };
        assert_relative_eq!(angle.value, degree!(45.).value);
        let Proptype::Vec2(dir) = parabola.node_attr.get_property("oa direction").unwrap() else {
            panic!()
        };
        assert_relative_eq!(*dir, Vector2::new(3., 2.).normalize());
    }
    #[test]
    fn calc_off_axis_isometry() {
        let parabola = ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(45.),
            Vector2::new(0., 1.),
        )
        .unwrap();
        let transform_mat = Matrix4::from_vec(vec![
            -0.,
            1.,
            -0.,
            -0.,
            -0.7071067811865475,
            -0.,
            -0.7071067811865476,
            -0.6035533905932736,
            -0.7071067811865476,
            0.,
            0.7071067811865475,
            -0.3964466094067264,
            0.,
            0.,
            0.,
            1.,
        ])
        .transpose();
        assert_relative_eq!(
            transform_mat,
            parabola
                .calc_off_axis_isometry()
                .unwrap()
                .get_transform()
                .to_matrix(),
            epsilon = 3. * f64::EPSILON
        );
    }

    #[test]
    fn calc_parent_focal_length() {
        let parabola = ParabolicMirror::new_with_off_axis(
            "Parabola",
            meter!(1.),
            true,
            degree!(90.),
            Vector2::new(0., 1.),
        )
        .unwrap();
        assert_relative_eq!(parabola.calc_parent_focal_length().unwrap().value, 0.5);
    }

    #[test]
    fn with_oap_angle() {
        let parabola = ParabolicMirror::default()
            .with_oap_angle(degree!(45.))
            .unwrap();
        let Proptype::Angle(angle) = parabola.node_attr.get_property("oa angle").unwrap() else {
            panic!()
        };
        assert_relative_eq!(angle.value, 45. / 180. * f64::consts::PI);
    }
    #[test]
    fn with_oap_direction() {
        let parabola = ParabolicMirror::default()
            .with_oap_direction(Vector2::new(0.35, 8.35))
            .unwrap();
        let Proptype::Vec2(oa_dir) = parabola.node_attr.get_property("oa direction").unwrap()
        else {
            panic!()
        };
        assert_relative_eq!(*oa_dir, Vector2::new(0.35, 8.35).normalize());
    }
    #[test]
    fn analysis_raytrace_empty_input() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Geometric(Rays::default());
        let input = LightResult::from([("input_1".into(), light_data)]);
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn analysis_raytrace_no_input() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Fourier;
        let input = LightResult::from([("output_1".into(), light_data)]);
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn analysis_raytrace_lightdata_energy() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).is_err());
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
    fn analysis_raytrace_no_iso() {
        let mut node = ParabolicMirror::default();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Hexapolar::new(millimeter!(1.), 3).unwrap(),
        )
        .unwrap();
        let light_data = LightData::Geometric(rays);
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).is_err());
    }
    #[test]
    fn analysis_raytrace() {
        let mut node = ParabolicMirror::default();
        node.set_isometry(Isometry::identity()).unwrap();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Hexapolar::new(millimeter!(1.), 3).unwrap(),
        )
        .unwrap();
        let light_data = LightData::Geometric(rays);
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).is_ok());
    }

    #[test]
    fn analysis_energy() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        let input = LightResult::from([("input_1".into(), light_data)]);
        let output = AnalysisEnergy::analyze(&mut node, input);
        assert!(output.is_ok());
        assert!(!output.unwrap().is_empty());
    }

    #[test]
    fn analysis_energy_no_input() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Fourier;
        let input = LightResult::from([("output_1".into(), light_data)]);
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn analysis_energy_lightdata_raytrace() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Geometric(Rays::default());
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisEnergy::analyze(&mut node, input).is_ok());
    }
    #[test]
    fn analysis_energy_lightdata_ghost_focus() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::GhostFocus(vec![Rays::default()]);
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisEnergy::analyze(&mut node, input).is_ok());
    }

    #[test]
    fn analysis_energy_lightdata_fourier() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::Fourier;
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(AnalysisEnergy::analyze(&mut node, input).is_ok());
    }

    #[test]
    fn analysis_ghost_focus_empty_input() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::GhostFocus(Vec::<Rays>::new());
        let input = light_result_to_light_rays(LightResult::from([("input_1".into(), light_data)]))
            .unwrap();
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            input,
            &GhostFocusConfig::default(),
            &mut Vec::<Rays>::new(),
            0,
        )
        .unwrap();
        assert!(output.values().last().unwrap().is_empty());
    }

    #[test]
    fn analysis_ghost_focus_no_input() {
        let mut node = ParabolicMirror::default();
        let light_data = LightData::GhostFocus(vec![Rays::default()]);
        let input =
            light_result_to_light_rays(LightResult::from([("output_1".into(), light_data)]))
                .unwrap();
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            input,
            &GhostFocusConfig::default(),
            &mut Vec::<Rays>::new(),
            0,
        )
        .unwrap();
        assert!(output.values().last().unwrap().is_empty());
    }

    #[test]
    fn analysis_ghost_focus_no_iso() {
        let mut node = ParabolicMirror::default();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Hexapolar::new(millimeter!(1.), 3).unwrap(),
        )
        .unwrap();
        let light_data = LightData::GhostFocus(vec![rays]);
        let input = light_result_to_light_rays(LightResult::from([("input_1".into(), light_data)]))
            .unwrap();
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            input,
            &GhostFocusConfig::default(),
            &mut Vec::<Rays>::new(),
            0,
        );
        assert!(output.is_err());
    }
    #[test]
    fn analysis_ghost_focus() {
        let mut node = ParabolicMirror::default();
        node.set_isometry(Isometry::new_along_z(millimeter!(10.0)).unwrap())
            .unwrap();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Hexapolar::new(millimeter!(1.), 3).unwrap(),
        )
        .unwrap();
        let light_data = LightData::GhostFocus(vec![rays]);
        let input = light_result_to_light_rays(LightResult::from([("input_1".into(), light_data)]))
            .unwrap();
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            input,
            &GhostFocusConfig::default(),
            &mut Vec::<Rays>::new(),
            0,
        );
        assert!(output.is_ok());
    }

    #[test]
    fn calc_node_position() {
        let mut node = ParabolicMirror::default();
        node.set_isometry(Isometry::identity()).unwrap();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.),
            joule!(1.),
            &Hexapolar::new(millimeter!(1.), 3).unwrap(),
        )
        .unwrap();
        let light_data = LightData::Geometric(rays);
        let input = LightResult::from([("input_1".into(), light_data)]);
        assert!(node
            .calc_node_position(input, &RayTraceConfig::default())
            .is_ok());
    }
}
