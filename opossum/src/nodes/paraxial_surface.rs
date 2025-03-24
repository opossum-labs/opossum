#![warn(missing_docs)]
//! A paraxial surface (ideal lens)
use crate::{
    analyzers::{
        energy::AnalysisEnergy,
        ghostfocus::AnalysisGhostFocus,
        raytrace::{AnalysisRayTrace, MissedSurfaceStrategy},
        GhostFocusConfig, RayTraceConfig,
    },
    error::{OpmResult, OpossumError},
    light_result::{LightRays, LightResult},
    lightdata::LightData,
    millimeter,
    optic_node::OpticNode,
    optic_ports::PortType,
    properties::Proptype,
    rays::Rays,
};
use log::warn;
use opm_macros_lib::OpmNode;
use uom::{num_traits::Zero, si::f64::Length};

use super::node_attr::NodeAttr;

/// Paraxial surface (=ideal lens)
///
/// This node models a (flat) paraxial surface with a given `focal length`. This corresponds to an ideal lens which is aberration free
/// and achromatic. A positive `focal length` corresponds to a focussing (convex) lens while a negative `focal length` represents a
/// defocussing (concave) lens.
///
/// The propagation is performed for [`LightData::Geometric`] only. For [`LightData::Energy`] this node is "transparent" which means
/// that the input data is simply forward unmodified to the output (such as a `Dummy` node).
///
/// ## Optical Ports
///   - Inputs
///     - `front`
///   - Outputs
///     - `rear`
///
/// ## Properties
///   - `name`
///   - `apertures`
///   - `inverted`
///   - `focal length`
#[derive(OpmNode, Debug, Clone)]
#[opm_node("palegreen")]
pub struct ParaxialSurface {
    node_attr: NodeAttr,
}
unsafe impl Send for ParaxialSurface {}
impl Default for ParaxialSurface {
    /// Create a default paraxial surface (ideal thin lens) with a focal length of 10 mm.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("paraxial surface");

        node_attr
            .create_property("focal length", "focal length", millimeter!(10.0).into())
            .unwrap();
        let mut ps = Self { node_attr };
        ps.update_surfaces().unwrap();
        ps
    }
}
impl ParaxialSurface {
    /// Create a new paraxial surface node of the given focal length.
    ///
    /// # Errors
    /// This function returns an error if
    ///  - the given `focal_length` is 0.0 or not finite.
    pub fn new(name: &str, focal_length: Length) -> OpmResult<Self> {
        if focal_length.is_zero() || !focal_length.is_normal() {
            return Err(OpossumError::Other(
                "focal length must be != 0.0 and finite".into(),
            ));
        }
        let mut parsurf = Self::default();
        parsurf.node_attr.set_name(name);
        parsurf
            .node_attr
            .set_property("focal length", focal_length.into())?;
        Ok(parsurf)
    }
}
impl OpticNode for ParaxialSurface {
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
}
impl AnalysisGhostFocus for ParaxialSurface {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Proptype::Length(focal_length) = self.node_attr.get_property("focal length")?.clone()
        else {
            return Err(OpossumError::Analysis("cannot read focal length".into()));
        };
        let Some(bouncing_rays) = incoming_data.get(in_port) else {
            let mut out_light_rays = LightRays::default();
            out_light_rays.insert(out_port.into(), Vec::<Rays>::new());
            return Ok(out_light_rays);
        };
        let mut rays = bouncing_rays.clone();

        let this = &mut *self;
        let rays_bundle: &mut Vec<Rays> = &mut rays;
        let optic_name = format!("'{}' ({})", this.name(), this.node_type());
        let mut apodized = false;
        let iso = this.effective_surface_iso(in_port)?;
        let Some(surf) = this.get_optic_surface_mut(in_port) else {
            return Err(OpossumError::Analysis("no surface found".into()));
        };

        for rays in &mut *rays_bundle {
            rays.refract_on_surface(surf, None, true, &MissedSurfaceStrategy::Ignore)?;

            rays.refract_paraxial(focal_length, &iso)?;

            apodized |= rays.apodize(surf.aperture(), &iso)?;
            if apodized {
                warn!("Rays have been apodized at input aperture of {}. Results might not be accurate.", optic_name);
            }
            surf.evaluate_fluence_of_ray_bundle(rays, config.fluence_estimator())?;
        }
        // merge all rays
        if let Some(ld) = this.get_light_data_mut() {
            if let LightData::GhostFocus(rays) = ld {
                for r in &*rays_bundle {
                    rays.push(r.clone());
                }
            }
            if let LightData::Geometric(rays) = ld {
                for r in &*rays_bundle {
                    rays.merge(r);
                }
            }
        } else {
            this.set_light_data(LightData::GhostFocus(rays_bundle.clone()));
        }

        let mut out_light_rays = LightRays::default();
        out_light_rays.insert(out_port.to_string(), rays);
        Ok(out_light_rays)
    }
}
impl AnalysisEnergy for ParaxialSurface {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(data) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        Ok(LightResult::from([(out_port.into(), data.clone())]))
    }
}
impl AnalysisRayTrace for ParaxialSurface {
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
            let Proptype::Length(focal_length) =
                self.node_attr.get_property("focal length")?.clone()
            else {
                return Err(OpossumError::Analysis("cannot read focal length".into()));
            };
            let iso = self.effective_surface_iso(in_port)?;
            if let Some(surf) = self.get_optic_surface_mut(in_port) {
                let refraction_intended = true;
                rays.refract_on_surface(
                    surf,
                    None,
                    refraction_intended,
                    config.missed_surface_strategy(),
                )?;
                rays.refract_paraxial(focal_length, &iso)?;
                if let Some(aperture) = self.ports().aperture(&PortType::Input, in_port) {
                    rays.apodize(aperture, &iso)?;
                    rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                } else {
                    return Err(OpossumError::OpticPort("input aperture not found".into()));
                };
                if let Some(aperture) = self.ports().aperture(&PortType::Output, out_port) {
                    rays.apodize(aperture, &iso)?;
                    rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                } else {
                    return Err(OpossumError::OpticPort("output aperture not found".into()));
                };

                let mut light_result = LightResult::default();
                light_result.insert(out_port.into(), LightData::Geometric(rays));
                Ok(light_result)
            } else {
                Err(OpossumError::Analysis("no surface found. Aborting".into()))
            }
        } else {
            Err(crate::error::OpossumError::Analysis(
                "No LightData::Geometric for analyzer type RayTrace".into(),
            ))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::RayTraceConfig, degree, joule, millimeter, nanometer,
        nodes::test_helper::test_helper::*, optic_ports::PortType, ray::Ray, rays::Rays,
        utils::geom_transformation::Isometry,
    };
    use approx::assert_relative_eq;
    use assert_matches::assert_matches;
    use nalgebra::Vector3;
    #[test]
    fn default() {
        let mut node = ParaxialSurface::default();
        assert_eq!(node.name(), "paraxial surface");
        assert_eq!(node.node_type(), "paraxial surface");
        assert_eq!(node.inverted(), false);
        assert!(node.properties().get("focal length").is_ok());
        assert_matches!(
            node.properties().get("focal length").unwrap(),
            Proptype::Length(_)
        );
        if let Ok(Proptype::Length(dist)) = node.properties().get("focal length") {
            assert_eq!(*dist, millimeter!(10.0));
        } else {
            assert!(false, "cannot read focal length");
        }
        assert_eq!(node.node_color(), "palegreen");
        assert!(node.as_group_mut().is_err());
    }
    #[test]
    fn new() {
        let node = ParaxialSurface::new("Test", millimeter!(100.0)).unwrap();
        assert_eq!(node.name(), "Test");
        if let Ok(Proptype::Length(dist)) = node.properties().get("focal length") {
            assert_eq!(dist, &millimeter!(100.0));
        } else {
            assert!(false, "cannot read focal length");
        }
        assert!(ParaxialSurface::new("Test", millimeter!(-1.0)).is_ok());
        assert!(ParaxialSurface::new("Test", millimeter!(0.0)).is_err());
        assert!(ParaxialSurface::new("Test", millimeter!(f64::NAN)).is_err());
        assert!(ParaxialSurface::new("Test", millimeter!(f64::INFINITY)).is_err());
        assert!(ParaxialSurface::new("Test", millimeter!(f64::NEG_INFINITY)).is_err());
    }
    #[test]
    fn node_type_readonly() {
        let mut node = ParaxialSurface::default();
        assert!(node.set_property("node_type", "other".into()).is_err());
    }
    #[test]
    fn inverted() {
        test_inverted::<ParaxialSurface>()
    }
    #[test]
    fn ports() {
        let node = ParaxialSurface::default();
        assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn set_aperture() {
        test_set_aperture::<ParaxialSurface>("input_1", "output_1");
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<ParaxialSurface>()
    }
    #[test]
    fn analyze_wrong_port() {
        let mut node = ParaxialSurface::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("output_1".into(), input_light.clone());
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_geometric_wrong_data_type() {
        test_analyze_wrong_data_type::<ParaxialSurface>("input_1");
    }
    #[test]
    fn analyze_geometric_no_isometry() {
        test_analyze_geometric_no_isometry::<ParaxialSurface>("input_1");
    }
    #[test]
    fn analyze_geometric_ok() {
        let mut node = ParaxialSurface::default();
        node.set_isometry(
            Isometry::new(millimeter!(0.0, 0.0, 10.0), degree!(0.0, 0.0, 0.0)).unwrap(),
        )
        .unwrap();
        let mut rays = Rays::default();
        rays.add_ray(
            Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), nanometer!(1000.0), joule!(1.0))
                .unwrap(),
        );
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.position(), millimeter!(0.0, 0.0, 10.0));
            let dir = Vector3::z();
            assert_eq!(ray.direction(), dir);
        } else {
            assert!(false, "could not get LightData");
        }
    }
    #[test]
    fn test_shifted_x() {
        let mut node = ParaxialSurface::new("test", millimeter!(10.)).unwrap();
        node.set_isometry(
            Isometry::new(millimeter!(10.0, 0.0, 10.0), degree!(0.0, 0.0, 0.0)).unwrap(),
        )
        .unwrap();
        let mut rays = Rays::default();
        rays.add_ray(
            Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), nanometer!(1000.0), joule!(1.0))
                .unwrap(),
        );
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();

        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.position(), millimeter!(0.0, 0.0, 10.0));
            assert_eq!(ray.direction(), Vector3::new(1., 0., 1.).normalize());
        } else {
            assert!(false, "could not get LightData");
        }
    }
    #[test]
    fn test_shifted_y() {
        let mut node = ParaxialSurface::new("test", millimeter!(10.)).unwrap();
        node.set_isometry(
            Isometry::new(millimeter!(0.0, 10.0, 10.0), degree!(0.0, 0.0, 0.0)).unwrap(),
        )
        .unwrap();
        let mut rays = Rays::default();
        rays.add_ray(
            Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), nanometer!(1000.0), joule!(1.0))
                .unwrap(),
        );
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();

        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.position(), millimeter!(0.0, 0.0, 10.0));
            assert_eq!(ray.direction(), Vector3::new(0., 1., 1.).normalize());
        } else {
            assert!(false, "could not get LightData");
        }
    }

    #[test]
    fn test_rotated_y() {
        let mut node = ParaxialSurface::new("test", millimeter!(10.)).unwrap();
        node.set_isometry(
            Isometry::new(millimeter!(0.0, 0.0, 10.0), degree!(45.0, 0.0, 0.0)).unwrap(),
        )
        .unwrap();
        let mut rays = Rays::default();
        rays.add_ray(
            Ray::new_collimated(
                millimeter!(0.0, 10.0 / f64::sqrt(2.), 0.0),
                nanometer!(1000.0),
                joule!(1.0),
            )
            .unwrap(),
        );
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();

        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_relative_eq!(ray.position()[0].value, 0.0);
            assert_relative_eq!(ray.position()[1].value, 0.01 / f64::sqrt(2.));
            assert_relative_eq!(ray.position()[2].value, 0.01 / f64::sqrt(2.) + 0.01);
            assert_relative_eq!(ray.direction(), Vector3::new(0., -1., 1.).normalize());
        } else {
            assert!(false, "could not get LightData");
        }
    }

    #[test]
    fn test_rotated_x() {
        let mut node = ParaxialSurface::new("test", millimeter!(10.)).unwrap();
        node.set_isometry(
            Isometry::new(millimeter!(0.0, 0.0, 10.0), degree!(0.0, 45.0, 0.0)).unwrap(),
        )
        .unwrap();
        let mut rays = Rays::default();
        rays.add_ray(
            Ray::new_collimated(
                millimeter!(-10.0 / f64::sqrt(2.), 0.0, 0.0),
                nanometer!(1000.0),
                joule!(1.0),
            )
            .unwrap(),
        );
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();

        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_relative_eq!(ray.position()[0].value, -0.01 / f64::sqrt(2.));
            assert_relative_eq!(ray.position()[1].value, 0.0);
            assert_relative_eq!(ray.position()[2].value, 0.01 / f64::sqrt(2.) + 0.01);
            assert_relative_eq!(ray.direction(), Vector3::new(1., 0., 1.).normalize());
        } else {
            assert!(false, "could not get LightData");
        }
    }

    #[test]
    fn as_ref_node_mut() {
        let mut node = ParaxialSurface::default();
        assert!(node.as_refnode_mut().is_err());
    }
}
