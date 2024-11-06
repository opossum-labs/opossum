use nalgebra::Point2;
use uom::si::f64::{Angle, Length};

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
            .create_property(
                "oap angle x",
                "off axis angle around local x axis",
                None,
                degree!(0.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "oap angle y",
                "off axis angle around local y axis",
                None,
                degree!(0.0).into(),
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
    /// Creates a new [`ParabolicMirror`] node.
    ///
    /// This function creates a infinitely thin parabolic mirror with a given focal length.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given focal length is zero or not finite.
    pub fn new(name: &str, focal_length: Length) -> OpmResult<Self> {
        if !focal_length.is_normal() {
            return Err(OpossumError::Other(
                "focal length must not be 0.0 and finite".into(),
            ));
        }
        let mut parabola = Self::default();
        parabola.node_attr.set_name(name);
        parabola
            .node_attr
            .set_property("focal length", focal_length.into())?;
        parabola.update_surfaces()?;
        Ok(parabola)
    }
    /// Returns / modifies a [`ParabolicMirror`] with given off-axis angles.
    ///
    /// The angles define the off axis angles around the local x and y axis of the node. The given angles denote the full
    /// angle between an incoming and a reflected beam. Effectively this introduces a decentering
    /// of the node during positioning in 3D space such that the desired angles are met.
    ///
    /// # Errors
    ///
    /// This function will return an error if the node properties cannot be set.
    pub fn with_oap_angles(mut self, angles: Point2<Angle>) -> OpmResult<Self> {
        self.set_property("oap angle x", angles[0].into())?;
        self.set_property("oap angle y", angles[1].into())?;
        self.update_surfaces()?;
        Ok(self)
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
        let Ok(Proptype::Length(focal_length)) = self.node_attr.get_property("focal length") else {
            return Err(OpossumError::Analysis("cannot read focal length".into()));
        };
        let Ok(Proptype::Angle(oap_angle_x)) = self.node_attr.get_property("oap angle x") else {
            return Err(OpossumError::Analysis(
                "cannot read off axis angle x".into(),
            ));
        };
        let Ok(Proptype::Angle(oap_angle_y)) = self.node_attr.get_property("oap angle y") else {
            return Err(OpossumError::Analysis(
                "cannot read off axis angle y".into(),
            ));
        };
        let mut parabola = Parabola::new(*focal_length, &Isometry::identity())?;
        parabola.set_off_axis_angles((*oap_angle_x, *oap_angle_y));
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
impl Analyzable for ParabolicMirror {}
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
                if let Some(surf) = self.get_optic_surface_mut(in_port) {
                    let refraction_intended = false;

                    surf.set_isometry(&iso);
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
