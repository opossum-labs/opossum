use core::f64;

use nalgebra::{vector, Isometry3, Point2, Point3, Rotation3, UnitQuaternion, Vector2, Vector3};
use num::Num;
use uom::si::{
    f64::{Angle, Length, Ratio},
    ratio,
};

use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
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
    surface::{geo_surface::GeometricSurface, optic_surface::OpticSurface, Parabola},
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
            .create_property("focal length", "focal length", None, meter!(1.0).into())
            .unwrap();
        node_attr
            .create_property("oa angle", "off axis angle", None, degree!(0.0).into())
            .unwrap();
        node_attr
            .create_property("collimating", "collimation flag. True if the parabola should collimate, false otherwise", None, false.into())
            .unwrap();
        
        node_attr
            .create_property(
                "oa direction",
                "off axis direction in the local coordinate system",
                None,
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
        parabola.set_properties(focal_length, collimating, None, None)?;
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
        Self::check_attributes(&focal_length, Some(&oa_angle), None)?;

        let mut parabola = Self::default();
        parabola.node_attr.set_name(name);
        parabola.set_properties(focal_length, collimating, Some(oa_angle), Some(Vector2::new(1., 0.)))?;
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
        Self::check_attributes(&focal_length, Some(&oa_angle), None)?;

        let mut parabola = Self::default();
        parabola.node_attr.set_name(name);
        parabola.set_properties(focal_length, collimating, Some(oa_angle), Some(Vector2::new(0., 1.)))?;

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
        Self::check_attributes(&focal_length, Some(&oa_angle), Some(&oa_dir))?;

        let mut parabola = Self::default();
        parabola.node_attr.set_name(name);
        parabola.set_properties(focal_length, collimating, Some(oa_angle), Some(oa_dir))?;

        parabola.update_surfaces()?;
        Ok(parabola)
    }

    /// checks the validity of the provided node attributes of thie parabola
    fn check_attributes(
        focal_length: &Length,
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
            if oa_angle.value.abs() > f64::consts::PI{
                return Err(OpossumError::Other("off-axis angle must be smaller than 180°".into()));
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
    fn set_properties(
        &mut self,
        focal_length: Length,
        collimating: bool,
        oa_angle_opt: Option<Angle>,
        oa_dir_opt: Option<Vector2<f64>>,
    ) -> OpmResult<()> {
        self.node_attr
            .set_property("focal length", focal_length.into())?;
        self.node_attr
            .set_property("collimating", collimating.into())?;

        if let Some(oa_angle) = oa_angle_opt {
            self.node_attr.set_property("oa angle", oa_angle.into())?;
        }

        if let Some(oa_dir) = oa_dir_opt {
            self.node_attr.set_property("oa direction", oa_dir.into())?;
        }
        Ok(())
    }

    fn calc_off_axis_isometry(&self) -> OpmResult<Isometry> {
        let (focal_length, oa_angle, oa_dir, collimating) = self.get_parabola_attributes()?;
        let tan_val = (oa_angle / 2.).tan().value;
        let decenter_x = oa_angle.sin() * focal_length;
        let z_shift = focal_length * (1. / (1. + tan_val * tan_val) - oa_angle.cos().value);
        let z_rot_angle = f64::atan2(oa_dir.y, oa_dir.x);

        let iso = Isometry::new_translation(Point3::new(decenter_x, meter!(0.), z_shift))?;
        let rot_iso = Isometry::new_rotation(radian!(0., 0., z_rot_angle))?;
        let mut tot_iso  =rot_iso.append(&iso);
        if collimating{
            let normal_vector = vector![decenter_x.value, 0., -2. * self.calc_parent_focal_length()?.value];
            let trans_normal_vector = tot_iso.transform_vector_f64(&normal_vector);
            let rot  = Isometry::new_from_transform(Isometry3::new(Vector3::zeros(), trans_normal_vector.normalize() * std::f64::consts::PI));
            tot_iso = rot.append(&tot_iso)
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
        Ok(*focal_length / (1. + tan_val * tan_val))
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
        self.set_property("oa direction", oa_dir.into())?;
        self.update_surfaces()?;
        Ok(self)
    }

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
            return Err(OpossumError::Analysis("cannot read collimation flag".into()));
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
        let iso = self.calc_off_axis_isometry()?;
        let parabola = Parabola::new(-1. * self.calc_parent_focal_length()?, &iso)?;
        let para_geo_surface = GeometricSurface::Parabolic { s: parabola };
        if let Some(optic_surf) = self
            .ports_mut()
            .get_optic_surface_mut(&"input_1".to_string())
        {
            optic_surf.set_geo_surface(para_geo_surface.clone());
        } else {
            let mut optic_surf_rear = OpticSurface::default();
            optic_surf_rear.set_geo_surface(para_geo_surface.clone());
            self.ports_mut()
                .add_optic_surface(&PortType::Input, "input_1", optic_surf_rear)?;
        }
        if let Some(optic_surf) = self
            .ports_mut()
            .get_optic_surface_mut(&"output_1".to_string())
        {
            optic_surf.set_geo_surface(para_geo_surface);
        } else {
            let mut optic_surf_rear = OpticSurface::default();
            optic_surf_rear.set_geo_surface(para_geo_surface);
            self.ports_mut()
                .add_optic_surface(&PortType::Output, "output_1", optic_surf_rear)?;
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
    fn set_surface_iso(&mut self, port_str: &str, iso: &Isometry) -> OpmResult<()> {
        let parabola_iso = self.calc_off_axis_isometry()?;
        if let Some(input_surf) = self.get_optic_surface_mut(port_str) {
            input_surf.set_isometry(&iso.append(&parabola_iso));
        } else {
            return Err(OpossumError::OpticPort("No surface found.".into()));
        }
        Ok(())
    }
}
impl AnalysisGhostFocus for ParabolicMirror {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        _config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];

        let mut rays_bundle = incoming_data
            .get(in_port)
            .map_or_else(Vec::<Rays>::new, std::clone::Clone::clone);

        for rays in &mut rays_bundle {
            let mut input = LightResult::default();
            input.insert(in_port.clone(), LightData::Geometric(rays.clone()));
            let out = AnalysisRayTrace::analyze(self, input, &RayTraceConfig::default())?;

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
            surf.evaluate_fluence_of_ray_bundle(rays)?;
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
        if let LightData::Geometric(mut rays) = data.clone() {
            let reflected = if let Some(iso) = self.effective_iso() {
                self.set_surface_iso(in_port, &iso)?;
                if let Some(surf) = self.get_optic_surface_mut(in_port) {
                    let refraction_intended = false;
                    let mut reflected_rays =
                        rays.refract_on_surface(surf, None, refraction_intended)?;
                    if let Some(aperture) = self.ports().aperture(&PortType::Input, in_port) {
                        reflected_rays.apodize(aperture, &iso)?;
                        reflected_rays
                            .invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                        reflected_rays
                    } else {
                        return Err(OpossumError::OpticPort("input aperture not found".into()));
                    }
                } else {
                    return Err(OpossumError::Analysis("no surface found. Aborting".into()));
                }
            } else {
                return Err(OpossumError::Analysis(
                    "no location for surface defined. Aborting".into(),
                ));
            };
            let light_data = LightData::Geometric(reflected);
            let light_result = LightResult::from([(out_port.into(), light_data)]);
            Ok(light_result)
        } else {
            Err(OpossumError::Analysis(
                "expected ray data at input port".into(),
            ))
        }
    }

    fn calc_node_position(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        AnalysisRayTrace::analyze(self, incoming_data, config)
    }
}
