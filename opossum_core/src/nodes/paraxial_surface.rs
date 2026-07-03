#![warn(missing_docs)]
//! A paraxial surface (ideal lens)
use crate::{
    analyzers::{
        GhostFocusConfig, RayTraceConfig, energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{NodeAttr, OpticNode, OpticNodeExt, PortType},
    error::{OpmResult, OpossumError},
    light::{LightData, LightRays, LightResult, Rays},
    millimeter,
    nodes::NodeRegistration,
    properties::{Proptype, validator::Validator},
};
use log::warn;
use opm_macros_lib::OpmNode;
use uom::si::f64::Length;

inventory::submit! {
    NodeRegistration::new::<ParaxialSurface>("paraxial surface", "ideal thin lens")
}

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

impl Default for ParaxialSurface {
    /// Create a default paraxial surface (ideal thin lens) with a focal length of 10 mm.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("paraxial surface");

        node_attr
            .create_property_with_validator(
                "focal length",
                "focal length",
                Validator::AndValidator {
                    validators: vec![Validator::NumericIsFinite, Validator::NumericIsNotZero],
                },
                millimeter!(10.0).into(),
            )
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
}
impl AnalysisGhostFocus for ParaxialSurface {
    fn analyze(
        &mut self,
        mut incoming_data: LightRays,
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
        let Some(mut rays_bundle) = incoming_data.remove(in_port) else {
            let mut out_light_rays = LightRays::default();
            out_light_rays.insert(out_port.into(), Vec::<Rays>::new());
            return Ok(out_light_rays);
        };
        let iso = self.effective_surface_iso(in_port)?;
        self.pass_through_surface_generic(in_port, None, &mut rays_bundle, config, false, false)?;
        for rays in &mut rays_bundle {
            rays.refract_paraxial(focal_length, &iso)?;
        }
        let mut out_light_rays = LightRays::default();
        out_light_rays.insert(out_port.clone(), rays_bundle);
        Ok(out_light_rays)
    }
}
impl AnalysisEnergy for ParaxialSurface {}
impl AnalysisRayTrace for ParaxialSurface {
    fn analyze(
        &mut self,
        mut incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(data) = incoming_data.remove(in_port) else {
            return Ok(LightResult::default());
        };
        let LightData::Geometric(rays) = data else {
            return Err(crate::error::OpossumError::Analysis(
                "No LightData::Geometric for analyzer type RayTrace".into(),
            ));
        };
        let Proptype::Length(focal_length) = self.node_attr.get_property("focal length")?.clone()
        else {
            return Err(OpossumError::Analysis("cannot read focal length".into()));
        };
        let iso = self.effective_surface_iso(in_port)?;
        let mut rays_bundle = vec![rays];
        self.pass_through_surface_generic(in_port, None, &mut rays_bundle, config, false, true)?;
        let rays = &mut rays_bundle[0];
        rays.refract_paraxial(focal_length, &iso)?;
        let mut light_result = LightResult::default();
        light_result.insert(out_port.into(), LightData::Geometric(rays_bundle.remove(0)));
        Ok(light_result)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::RayTraceConfig,
        core_optics::{NodeAttrExt, PortType},
        degree, joule,
        light::{Ray, Rays},
        millimeter, nanometer,
        nodes::test_helper::test_helper::*,
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
    fn new() -> OpmResult<()> {
        let node = ParaxialSurface::new("Test", millimeter!(100.0))?;
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
        Ok(())
    }
    #[test]
    fn node_type_readonly() {
        let mut node = ParaxialSurface::default();
        assert!(node.set_property("node_type", "other".into()).is_err());
    }
    #[test]
    fn inverted() -> OpmResult<()> {
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
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<ParaxialSurface>()
    }
    #[test]
    fn analyze_wrong_port() -> OpmResult<()> {
        let mut node = ParaxialSurface::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    #[test]
    fn analyze_geometric_wrong_data_type() -> OpmResult<()> {
        test_analyze_wrong_data_type::<ParaxialSurface>("input_1")
    }
    #[test]
    fn analyze_geometric_no_isometry() {
        test_analyze_geometric_no_isometry::<ParaxialSurface>("input_1");
    }
    #[test]
    fn analyze_geometric_ok() -> OpmResult<()> {
        let mut node = ParaxialSurface::default();
        node.set_isometry(Isometry::new(
            millimeter!(0.0, 0.0, 10.0),
            degree!(0.0, 0.0, 0.0),
        )?)?;
        let mut rays = Rays::default();
        let mut initial_ray =
            Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), nanometer!(1000.0), joule!(1.0))?;
        initial_ray.add_to_pos_hist(millimeter!(0., 0., -10.));
        rays.add_ray(initial_ray);

        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;

        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.position(), millimeter!(0.0, 0.0, 10.0));
            let dir = Vector3::z();
            assert_eq!(ray.direction(), dir);
            assert!(
                ray.ray_history_len() > 1,
                "Ray position history was lost or not updated during propagation!"
            );
        } else {
            assert!(false, "could not get LightData");
        }
        Ok(())
    }
    #[test]
    fn test_shifted_x() -> OpmResult<()> {
        let mut node = ParaxialSurface::new("test", millimeter!(10.))?;
        node.set_isometry(Isometry::new(
            millimeter!(10.0, 0.0, 10.0),
            degree!(0.0, 0.0, 0.0),
        )?)?;
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0.0, 0.0, 0.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;

        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.position(), millimeter!(0.0, 0.0, 10.0));
            assert_eq!(ray.direction(), Vector3::new(1., 0., 1.).normalize());
        } else {
            assert!(false, "could not get LightData");
        }
        Ok(())
    }
    #[test]
    fn test_shifted_y() -> OpmResult<()> {
        let mut node = ParaxialSurface::new("test", millimeter!(10.))?;
        node.set_isometry(Isometry::new(
            millimeter!(0.0, 10.0, 10.0),
            degree!(0.0, 0.0, 0.0),
        )?)?;
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0.0, 0.0, 0.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;

        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.position(), millimeter!(0.0, 0.0, 10.0));
            assert_eq!(ray.direction(), Vector3::new(0., 1., 1.).normalize());
        } else {
            assert!(false, "could not get LightData");
        }
        Ok(())
    }

    #[test]
    fn test_rotated_y() -> OpmResult<()> {
        let mut node = ParaxialSurface::new("test", millimeter!(10.))?;
        node.set_isometry(Isometry::new(
            millimeter!(0.0, 0.0, 10.0),
            degree!(45.0, 0.0, 0.0),
        )?)?;
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0.0, 10.0 / f64::sqrt(2.), 0.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;

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
        Ok(())
    }

    #[test]
    fn test_rotated_x() -> OpmResult<()> {
        let mut node = ParaxialSurface::new("test", millimeter!(10.))?;
        node.set_isometry(Isometry::new(
            millimeter!(0.0, 0.0, 10.0),
            degree!(0.0, 45.0, 0.0),
        )?)?;
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(-10.0 / f64::sqrt(2.), 0.0, 0.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;

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
        Ok(())
    }

    #[test]
    fn as_ref_node_mut() {
        let mut node = ParaxialSurface::default();
        assert!(node.as_refnode_mut().is_err());
    }
}
