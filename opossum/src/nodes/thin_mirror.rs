#![warn(missing_docs)]
//! Infinitely thin mirror with spherical or flat surface
use std::{cell::RefCell, rc::Rc};

use super::NodeAttr;
use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
        Analyzable, GhostFocusConfig, RayTraceConfig,
    },
    coatings::CoatingType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    light_result::{LightRays, LightResult},
    lightdata::LightData,
    meter, millimeter,
    optic_node::{Alignable, OpticNode, LIDT},
    optic_ports::PortType,
    properties::Proptype,
    radian,
    rays::Rays,
    surface::{geo_surface::GeoSurfaceRef, Plane, Sphere},
    utils::geom_transformation::Isometry,
};
use num::Zero;
use uom::si::f64::Length;

#[derive(Debug, Clone)]
/// An infinitely thin mirror with a spherical (or flat) surface.
///
/// Curvature convention:
/// - negative curvature will be a concave (focusing) mirror
/// - positive curvature will be a convex (defocusing) mirror
/// ## Optical Ports
///   - Inputs
///     - `in1`
///   - Outputs
///     - `out1`
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `curvature`
pub struct ThinMirror {
    node_attr: NodeAttr,
}
impl Default for ThinMirror {
    /// Create a thin mirror with a flat surface.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("mirror");
        node_attr
            .create_property(
                "curvature",
                "radius of curvature of the surface",
                None,
                millimeter!(f64::INFINITY).into(),
            )
            .unwrap();

        let mut m = Self { node_attr };
        m.update_surfaces().unwrap();
        m.ports_mut()
            .set_coating(
                &PortType::Input,
                "input_1",
                &CoatingType::ConstantR { reflectivity: 1.0 },
            )
            .unwrap();

        m.ports_mut()
            .set_coating(
                &PortType::Output,
                "output_1",
                &CoatingType::ConstantR { reflectivity: 1.0 },
            )
            .unwrap();
        m
    }
}
impl ThinMirror {
    /// Creates a new [`ThinMirror`].
    ///
    /// This function creates a infinitely thin mirror with a flat surface. A spherical mirror can be modelled by appending the
    /// function `with_curvature`.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut mirror = Self::default();
        mirror.node_attr.set_name(name);
        mirror
    }
    /// Modifies a [`ThinMirror`]'s curvature.
    ///
    /// The given radius of curvature must not be zero. A radius of curvature of +/- infinity
    /// corresponds to a flat surface. This function can be used with the "builder pattern".
    ///
    /// # Errors
    ///
    /// This function will return an error if the given radius of curvature is zero or not finite.
    pub fn with_curvature(mut self, curvature: Length) -> OpmResult<Self> {
        if curvature.is_zero() || curvature.is_nan() {
            return Err(OpossumError::Other(
                "curvature must not be 0.0 or NaN".into(),
            ));
        }
        self.node_attr.set_property("curvature", curvature.into())?;
        self.update_surfaces()?;
        Ok(self)
    }
}
impl OpticNode for ThinMirror {
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        let node_iso = self.effective_node_iso().unwrap_or_else(Isometry::identity);
        let Ok(Proptype::Length(curvature)) = self.node_attr.get_property("curvature") else {
            return Err(OpossumError::Analysis("cannot read curvature".into()));
        };
        let (geosurface, anchor_point_iso) = if curvature.is_infinite() {
            (
                GeoSurfaceRef(Rc::new(RefCell::new(Plane::new(&node_iso)))),
                Isometry::identity(),
            )
        } else {
            let anchor_point_iso_front =
                Isometry::new(meter!(0., 0., curvature.value), radian!(0., 0., 0.))?;
            (
                GeoSurfaceRef(Rc::new(RefCell::new(Sphere::new(
                    *curvature,
                    node_iso.append(&anchor_point_iso_front),
                )?))),
                anchor_point_iso_front,
            )
        };

        self.update_surface(
            &"input_1".to_string(),
            geosurface.clone(),
            anchor_point_iso.clone(),
            &PortType::Input,
        )?;
        self.update_surface(
            &"output_1".to_string(),
            geosurface,
            anchor_point_iso,
            &PortType::Output,
        )?;

        Ok(())
    }
    #[cfg(feature = "bevy")]
    fn mesh(&self) -> Mesh {
        #[allow(clippy::cast_possible_truncation)]
        let thickness = if let Ok(Proptype::Length(center_thickness)) =
            self.node_attr.get_property("center thickness")
        {
            center_thickness.value as f32
        } else {
            warn!("could not read center thickness. using 0.001 as default");
            0.001_f32
        };
        let mesh: Mesh = Cuboid::new(0.3, 0.3, thickness).into();
        if let Some(iso) = self.effective_iso() {
            mesh.transformed_by(iso.into())
        } else {
            warn!("Node has no isometry defined. Mesh will be located at origin.");
            mesh
        }
    }
}

impl Alignable for ThinMirror {}

impl Dottable for ThinMirror {
    fn node_color(&self) -> &str {
        "aliceblue"
    }
}
impl LIDT for ThinMirror {}
impl Analyzable for ThinMirror {}
impl AnalysisGhostFocus for ThinMirror {
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
impl AnalysisEnergy for ThinMirror {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(data) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        Ok(LightResult::from([(out_port.into(), data.clone())]))
    }
}
impl AnalysisRayTrace for ThinMirror {
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
            let reflected = if let Some(surf) = self.get_optic_surface_mut(in_port) {
                let refraction_intended = false;
                let mut reflected_rays =
                    rays.refract_on_surface(surf, None, refraction_intended)?;
                if let Some(aperture) = self.ports().aperture(&PortType::Input, in_port) {
                    reflected_rays.apodize(aperture, &self.effective_surface_iso(in_port)?)?;
                    reflected_rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                    reflected_rays
                } else {
                    return Err(OpossumError::OpticPort("input aperture not found".into()));
                }
            } else {
                return Err(OpossumError::Analysis("no surface found. Aborting".into()));
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::RayTraceConfig, degree, joule, lightdata::DataEnergy, nanometer,
        nodes::test_helper::test_helper::*, optic_ports::PortType, ray::Ray, rays::Rays,
        spectrum_helper::create_he_ne_spec, utils::geom_transformation::Isometry,
    };
    use nalgebra::vector;
    #[test]
    fn default() {
        let node = ThinMirror::default();
        assert_eq!(node.name(), "mirror");
        assert_eq!(node.node_type(), "mirror");
        assert_eq!(node.node_color(), "aliceblue");
        assert_eq!(node.inverted(), false);
        if let Ok(Proptype::Length(r)) = node.properties().get("curvature") {
            assert_eq!(r, &millimeter!(f64::INFINITY));
        } else {
            assert!(false, "property curvature was not a length.");
        }
    }
    #[test]
    fn new() {
        let m = ThinMirror::new("test");
        assert_eq!(m.name(), "test");
        assert_eq!(m.node_type(), "mirror");
        if let Ok(Proptype::Length(r)) = m.properties().get("curvature") {
            assert_eq!(r, &millimeter!(f64::INFINITY));
        } else {
            assert!(false, "property curvature was not a length.");
        }
    }
    #[test]
    fn ports() {
        let node = ThinMirror::default();
        assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn set_aperture() {
        test_set_aperture::<ThinMirror>("input_1", "output_1");
    }
    #[test]
    fn inverted() {
        test_inverted::<ThinMirror>()
    }
    #[test]
    fn with_curvature() {
        assert!(ThinMirror::default()
            .with_curvature(Length::zero())
            .is_err());
        assert!(ThinMirror::default()
            .with_curvature(millimeter!(f64::NAN))
            .is_err());
        assert!(ThinMirror::default()
            .with_curvature(millimeter!(f64::INFINITY))
            .is_ok());
        assert!(ThinMirror::default()
            .with_curvature(millimeter!(f64::NEG_INFINITY))
            .is_ok());
        let m = ThinMirror::default()
            .with_curvature(millimeter!(100.0))
            .unwrap();
        if let Ok(Proptype::Length(r)) = m.properties().get("curvature") {
            assert_eq!(r, &millimeter!(100.0));
        } else {
            assert!(false, "property curvature was not a length.");
        }
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<ThinMirror>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = ThinMirror::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_energy_ok() {
        let mut node = ThinMirror::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_geometric_wrong_data_type() {
        test_analyze_wrong_data_type::<ThinMirror>("input_1");
    }
    #[test]
    fn analyze_geometric_no_isometery() {
        test_analyze_geometric_no_isometry::<ThinMirror>("input_1");
    }
    #[test]
    fn analyze_geometric_ok() {
        let mut node = ThinMirror::default();

        node.set_isometry(
            Isometry::new(millimeter!(0.0, 0.0, 10.0), degree!(0.0, 0.0, 0.0)).unwrap(),
        )
        .unwrap();
        let mut input = LightResult::default();
        let mut rays = Rays::default();
        rays.add_ray(Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap());
        let input_light = LightData::Geometric(rays);
        input.insert("input_1".into(), input_light.clone());
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.position(), millimeter!(0.0, 0.0, 10.0));
            let dir = vector![0.0, 0.0, -1.0];
            assert_eq!(ray.direction(), dir);
        } else {
            assert!(false, "could not get LightData");
        }
    }
}
