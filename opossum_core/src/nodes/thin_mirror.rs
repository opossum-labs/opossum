#![warn(missing_docs)]
//! Infinitely thin mirror with spherical or flat surface
use crate::{
    analyzers::{
        GhostFocusConfig, RayTraceConfig, energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus,
        propagation_strategy::MissedSurfaceStrategy, raytrace::AnalysisRayTrace,
    },
    coatings::CoatingType,
    core_optics::{NodeAttr, OpticNode, PortType},
    error::{OpmResult, OpossumError},
    geometry::{Plane, Sphere, geo_surface::GeoSurfaceRef},
    light::{LightData, LightResult, Rays, light_result::LightRays},
    meter, millimeter,
    nodes::NodeRegistration,
    properties::{Proptype, validator::Validator},
    radian,
    utils::geom_transformation::Isometry,
};
use opm_macros_lib::OpmNode;
use std::sync::{Arc, Mutex};
use uom::si::f64::Length;

inventory::submit! {
    NodeRegistration::new::<ThinMirror>("mirror", "ideal flat/spherical mirror")
}

#[derive(OpmNode, Debug, Clone)]
#[opm_node("aliceblue")]
/// An infinitely thin mirror with a spherical (or flat) surface.
///
/// Curvature convention:
/// - negative curvature will be a concave (focusing) mirror
/// - positive curvature will be a convex (defocusing) mirror
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
///   - `curvature`
pub struct ThinMirror {
    node_attr: NodeAttr,
}
unsafe impl Send for ThinMirror {}

impl Default for ThinMirror {
    /// Create a thin mirror with a flat surface.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("mirror");
        node_attr
            .create_property_with_validator(
                "curvature",
                "radius of curvature of the surface",
                // and_validator(vec![numeric_is_not_zero(), numeric_is_not_nan()]),
                Validator::AndValidator {
                    validators: vec![Validator::NumericIsNotNaN, Validator::NumericIsNotZero],
                },
                Proptype::Curvature(millimeter!(f64::INFINITY)),
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
        self.node_attr
            .set_property("curvature", Proptype::Curvature(curvature))?;
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
        let Ok(Proptype::Curvature(curvature)) = self.node_attr.get_property("curvature") else {
            return Err(OpossumError::Analysis("cannot read curvature".into()));
        };
        let (geosurface, anchor_point_iso) = if curvature.is_infinite() {
            (
                GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(node_iso)))),
                Isometry::identity(),
            )
        } else {
            let anchor_point_iso_front =
                Isometry::new(meter!(0., 0., curvature.value), radian!(0., 0., 0.))?;
            (
                GeoSurfaceRef(Arc::new(Mutex::new(Sphere::new(
                    *curvature,
                    node_iso.append(&anchor_point_iso_front),
                )?))),
                anchor_point_iso_front,
            )
        };
        self.update_surface(
            &"input_1".to_string(),
            geosurface.clone(),
            anchor_point_iso,
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
}
impl AnalysisGhostFocus for ThinMirror {
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
impl AnalysisEnergy for ThinMirror {}
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
                let mut reflected_rays = rays.refract_on_surface(
                    surf,
                    None,
                    refraction_intended,
                    config.missed_surface_strategy(),
                )?;
                match self.ports().aperture(&PortType::Input, in_port) {
                    Some(aperture) => {
                        reflected_rays.apodize(aperture, &self.effective_surface_iso(in_port)?)?;
                        reflected_rays
                            .invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                        reflected_rays
                    }
                    _ => {
                        return Err(OpossumError::OpticPort("input aperture not found".into()));
                    }
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
    use super::*;
    use crate::{
        analyzers::{RayTraceConfig, energy::EnergyConfig},
        core_optics::PortType,
        degree, joule,
        light::spectrum_helper::create_he_ne_spec,
        light::{Ray, Rays},
        nanometer,
        nodes::test_helper::test_helper::*,
        utils::geom_transformation::Isometry,
    };
    use nalgebra::vector;
    use num::Zero;
    #[test]
    fn default() {
        let node = ThinMirror::default();
        assert_eq!(node.name(), "mirror");
        assert_eq!(node.node_type(), "mirror");
        assert_eq!(node.node_color(), "aliceblue");
        assert_eq!(node.inverted(), false);
        if let Ok(Proptype::Curvature(r)) = node.properties().get("curvature") {
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
        if let Ok(Proptype::Curvature(r)) = m.properties().get("curvature") {
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
    fn inverted() -> OpmResult<()> {
        test_inverted::<ThinMirror>()?;
        Ok(())
    }
    #[test]
    fn with_curvature() -> OpmResult<()> {
        assert!(
            ThinMirror::default()
                .with_curvature(Length::zero())
                .is_err()
        );
        assert!(
            ThinMirror::default()
                .with_curvature(millimeter!(f64::NAN))
                .is_err()
        );
        assert!(
            ThinMirror::default()
                .with_curvature(millimeter!(f64::INFINITY))
                .is_ok()
        );
        assert!(
            ThinMirror::default()
                .with_curvature(millimeter!(f64::NEG_INFINITY))
                .is_ok()
        );
        let m = ThinMirror::default().with_curvature(millimeter!(100.0))?;
        if let Ok(Proptype::Curvature(r)) = m.properties().get("curvature") {
            assert_eq!(r, &millimeter!(100.0));
        } else {
            assert!(false, "property curvature was not a length.");
        }
        Ok(())
    }
    #[test]
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<ThinMirror>()?;
        Ok(())
    }
    #[test]
    fn analyze_wrong() -> OpmResult<()> {
        let mut node = ThinMirror::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    #[test]
    fn analyze_energy_ok() -> OpmResult<()> {
        let mut node = ThinMirror::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.ok_or_else(|| OpossumError::Other("got empty output".to_string()))?;
        assert_eq!(*output, input_light);
        Ok(())
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
    fn analyze_geometric_ok() -> OpmResult<()> {
        let mut node = ThinMirror::default();

        node.set_isometry(Isometry::new(
            millimeter!(0.0, 0.0, 10.0),
            degree!(0.0, 0.0, 0.0),
        )?)?;
        let mut input = LightResult::default();
        let rays = Rays::from(Ray::origin_along_z(nanometer!(1000.0), joule!(1.0))?);
        let input_light = LightData::Geometric(rays);
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays
                .iter()
                .next()
                .ok_or_else(|| OpossumError::Other("no rays in bundle found".to_string()))?;
            assert_eq!(ray.position(), millimeter!(0.0, 0.0, 10.0));
            let dir = vector![0.0, 0.0, -1.0];
            assert_eq!(ray.direction(), dir);
        } else {
            assert!(false, "could not get LightData");
        }
        Ok(())
    }
}
