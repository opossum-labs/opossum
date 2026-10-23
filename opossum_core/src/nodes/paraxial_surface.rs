#![warn(missing_docs)]
//! A paraxial surface (ideal lens)
use crate::{
    analyzers::{
        GhostFocusConfig, RayTraceConfig, energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    coatings::CoatingType,
    core_optics::{
        NodeAttr, OpticNode, OpticNodeExt, PortType, SurfacePorts,
        hit_map::rays_hit_map::HitPoint,
        surface_effect::{Refraction, SurfaceEffect},
    },
    error::{OpmResult, OpossumError},
    geometry::{
        Geometry, SurfaceHit,
        body::{OPTIC_SHAPE, default_optic_shape},
    },
    light::{LightData, LightRays, LightResult, Ray, Rays, rays::ReflectedPart},
    millimeter,
    nodes::NodeRegistration,
    properties::{Proptype, validator::Validator},
    utils::geom_transformation::Isometry,
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
///   - `optic shape`: the extent of the lens; a ray beyond it misses the lens, is not
///     deflected and follows the analyzer's missed surface strategy
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
        node_attr
            .create_property(
                OPTIC_SHAPE,
                "transversal extent of the surface, open for an unbounded ideal surface",
                default_optic_shape().into(),
            )
            .unwrap();
        let mut ps = Self { node_attr };
        ps.create_default_surface_properties().unwrap();
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
    /// Returns the ideal lens this node is at its input surface: its focal length, at the frame of
    /// that surface.
    ///
    /// # Arguments
    ///
    /// * `in_port` - the input port, whose surface the lens is.
    ///
    /// # Errors
    ///
    /// This function returns an error if the focal length cannot be read or the node is not placed.
    fn lens(&self, in_port: &str) -> OpmResult<ParaxialLens> {
        let Proptype::Length(focal_length) = self.node_attr.get_property("focal length")?.clone()
        else {
            return Err(OpossumError::Analysis("cannot read focal length".into()));
        };
        Ok(ParaxialLens {
            focal_length,
            frame: self.effective_surface_iso(in_port)?,
        })
    }
}

/// An ideal thin lens at a surface: a passive surface whose transmitted part is deflected as by a
/// lens of the given focal length (see [`Ray::refract_paraxial`]).
struct ParaxialLens {
    focal_length: Length,
    frame: Isometry,
}

impl SurfaceEffect for ParaxialLens {
    fn act_at(
        &self,
        ray: &mut Ray,
        hit: &SurfaceHit,
        coating: &CoatingType,
    ) -> OpmResult<(Option<Ray>, Option<HitPoint>)> {
        let split = Refraction(None).act_at(ray, hit, coating)?;
        ray.refract_paraxial(self.focal_length, &self.frame)?;
        Ok(split)
    }
}
impl OpticNode for ParaxialSurface {
    fn create_geometry(&self) -> OpmResult<Option<Geometry>> {
        Ok(Some(Geometry::plane(self.cross_section()?)))
    }
    fn surface_ports(&self) -> SurfacePorts {
        SurfacePorts::SINGLE_SURFACE
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

        let Some(mut rays_bundle) = incoming_data.remove(in_port) else {
            let mut out_light_rays = LightRays::default();
            out_light_rays.insert(out_port.into(), Vec::<Rays>::new());
            return Ok(out_light_rays);
        };
        let lens = self.lens(in_port)?;
        self.pass_through_surface_generic(
            in_port,
            &lens,
            &mut rays_bundle,
            config,
            false,
            ReflectedPart::Main,
        )?;
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
        let lens = self.lens(in_port)?;
        let mut rays_bundle = vec![rays];
        self.pass_through_surface_generic(
            in_port,
            &lens,
            &mut rays_bundle,
            config,
            false,
            ReflectedPart::Ghost,
        )?;
        let mut light_result = LightResult::default();
        light_result.insert(out_port.into(), LightData::Geometric(rays_bundle.remove(0)));
        Ok(light_result)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        J_per_cm2,
        analyzers::{RayTraceConfig, propagation_strategy::MissedSurfaceStrategy},
        core_optics::{NodeAttrExt, PortType, node_attr::NodePositioning},
        degree,
        geometry::SurfaceGeometry,
        joule,
        light::{Ray, Rays},
        millimeter, nanometer,
        nodes::test_helper::helper::*,
        utils::geom_transformation::Isometry,
    };
    use approx::assert_relative_eq;
    use assert_matches::assert_matches;
    use nalgebra::Vector3;
    #[test]
    fn default() -> OpmResult<()> {
        let node = ParaxialSurface::default();
        assert_eq!(node.name(), "paraxial surface");
        assert_eq!(node.node_type(), "paraxial surface");
        assert!(!node.inverted());
        assert!(node.properties().get("focal length").is_ok());
        assert_matches!(
            node.properties().get("focal length").unwrap(),
            Proptype::Length(_)
        );
        if let Proptype::Length(dist) = node.properties().get("focal length")? {
            assert_eq!(*dist, millimeter!(10.0));
        } else {
            panic!("cannot read focal length");
        }
        assert_eq!(node.node_color(), "palegreen");
        Ok(())
    }
    #[test]
    fn new() -> OpmResult<()> {
        let node = ParaxialSurface::new("Test", millimeter!(100.0))?;
        assert_eq!(node.name(), "Test");
        if let Ok(Proptype::Length(dist)) = node.properties().get("focal length") {
            assert_eq!(dist, &millimeter!(100.0));
        } else {
            panic!("cannot read focal length");
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
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<ParaxialSurface>()
    }
    #[test]
    fn optic_shape_absent_in_file() -> OpmResult<()> {
        test_optic_shape_absent_in_file::<ParaxialSurface>()
    }
    /// Beyond its optic shape an ideal lens neither focuses nor deflects: a ray trace loses the
    /// ray there, a ghost focus analysis lets it run past unchanged.
    #[test]
    fn rays_beyond_optic_shape_are_lost() -> OpmResult<()> {
        test_rays_beyond_optic_shape_are_lost::<ParaxialSurface>()
    }
    /// An ideal lens may be unbounded: with an open optic shape it focuses a ray however far off
    /// the axis it arrives.
    #[test]
    fn an_open_optic_shape_makes_an_unbounded_ideal_lens() -> OpmResult<()> {
        let mut node = placed_at_origin::<ParaxialSurface>()?;
        node.set_property(
            crate::geometry::body::OPTIC_SHAPE,
            crate::apertures::OpticShape::Open.into(),
        )?;
        node.update_surfaces()?;
        let far_off = rays_along_z_from(&[millimeter!(0.0, 100.0, -10.0)], nanometer!(1000.0))?;
        let output = AnalysisRayTrace::analyze(
            &mut node,
            LightResult::from([("input_1".into(), LightData::Geometric(far_off))]),
            &RayTraceConfig::default(),
        )?;
        let Some(LightData::Geometric(rays)) = output.get("output_1") else {
            panic!("expected ray data at the output port");
        };
        let ray = rays.iter().next().expect("one ray");
        assert!(ray.valid(), "the ray far off the axis passes the lens");
        // heading for the focus 10 mm behind the lens on its axis
        assert_relative_eq!(ray.direction(), Vector3::new(0.0, -100.0, 10.0).normalize());
        Ok(())
    }
    /// An ideal lens of 100 mm at a plane through the origin, bounded by the default optic shape.
    fn lens_at_origin() -> OpmResult<(Geometry, ParaxialLens)> {
        let plane = Geometry::plane(default_optic_shape().outline().cloned());
        let lens = ParaxialLens {
            focal_length: millimeter!(100.0),
            frame: Isometry::identity(),
        };
        Ok((plane, lens))
    }
    /// A ray on the axis passes the lens straight, a ray 1 mm off it heads for the focus.
    #[test]
    fn a_lens_deflects_a_ray_towards_its_focus() -> OpmResult<()> {
        let (mut plane, lens) = lens_at_origin()?;
        let ray0 = Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(1.0))?;
        let ray1 = Ray::new_collimated(millimeter!(0., 1., 0.), nanometer!(1053.0), joule!(1.0))?;
        let mut rays = Rays::from(vec![ray0.clone(), ray1.clone()]);
        plane.interact_with_rays(
            SurfaceGeometry::FACE,
            &mut rays,
            &lens,
            ReflectedPart::Ghost,
            MissedSurfaceStrategy::Stop,
        )?;
        let (passed0, passed1) = (rays.iter().next(), rays.iter().nth(1));
        let (passed0, passed1) = (passed0.expect("ray 0"), passed1.expect("ray 1"));
        assert_eq!(passed0.position(), ray0.position());
        assert_eq!(passed0.direction(), ray0.direction());
        assert_eq!(passed1.position(), ray1.position());
        assert_relative_eq!(
            passed1.direction(),
            Vector3::new(0.0, -1.0, 100.0).normalize()
        );
        Ok(())
    }
    /// A ray that hits the lens takes its helper rays along; a ray that misses it keeps its
    /// direction, and so do its helper rays.
    #[test]
    fn a_lens_deflects_the_helper_rays_of_a_hit_and_leaves_a_miss_alone() -> OpmResult<()> {
        let (mut plane, lens) = lens_at_origin()?;
        let with_helpers = |y: f64| {
            Ray::new_collimated_w_fluence_helper(
                millimeter!(0.0, y, -10.0),
                nanometer!(1000.0),
                joule!(1.0),
                J_per_cm2!(1.0),
            )
        };
        let mut rays = Rays::from(vec![with_helpers(5.0)?, with_helpers(20.0)?]);
        plane.interact_with_rays(
            SurfaceGeometry::FACE,
            &mut rays,
            &lens,
            ReflectedPart::Ghost,
            MissedSurfaceStrategy::Ignore,
        )?;
        for (ray, deflected) in rays.iter_mut().zip([true, false]) {
            assert_eq!(ray.direction() != Vector3::z(), deflected);
            let helpers = ray.helper_rays_mut().expect("a ray with helper rays");
            assert_eq!(helpers.nr_of_rays(false), 3, "a ray with three helper rays");
            for helper in helpers.iter() {
                assert_eq!(helper.direction() != Vector3::z(), deflected);
            }
        }
        Ok(())
    }
    /// Where a missed ray passes the lens unchanged, so does a missed helper ray: a ray 0.5 µm inside
    /// the 12.5 mm optic shape has one helper ray 0.38 µm beyond it, which is not deflected, while
    /// the ray and its other two helper rays are focused.
    #[test]
    fn a_helper_ray_beyond_the_optic_shape_passes_undeflected() -> OpmResult<()> {
        let mut node = placed_at_origin::<ParaxialSurface>()?;
        node.set_property(OPTIC_SHAPE, default_optic_shape().into())?;
        node.update_surfaces()?;
        let ray = Ray::new_collimated_w_fluence_helper(
            millimeter!(12.4995, 0.0, -10.0),
            nanometer!(1000.0),
            joule!(1.0),
            J_per_cm2!(1.0),
        )?;
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            LightRays::from([("input_1".into(), vec![Rays::from(ray)])]),
            &GhostFocusConfig::default(),
            &mut Vec::new(),
            0,
        )?;
        let mut ray = output
            .get("output_1")
            .and_then(|bundles| bundles.first())
            .and_then(|bundle| bundle.iter().next())
            .cloned()
            .expect("the ray at the output port");
        assert_ne!(ray.direction(), Vector3::z(), "the ray inside is focused");
        let helpers = ray
            .helper_rays_mut()
            .expect("the ray keeps its helper rays");
        let deflected: Vec<bool> = helpers
            .iter()
            .map(|helper| helper.direction() != Vector3::z())
            .collect();
        assert_eq!(deflected, [false, true, true]);
        Ok(())
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
        node.set_positioning(NodePositioning::Absolute(Isometry::new(
            millimeter!(0.0, 0.0, 10.0),
            degree!(0.0, 0.0, 0.0),
        )?))?;
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
            panic!("could not get LightData");
        }
        Ok(())
    }
    #[test]
    fn test_shifted_x() -> OpmResult<()> {
        let mut node = ParaxialSurface::new("test", millimeter!(10.))?;
        node.set_positioning(NodePositioning::Absolute(Isometry::new(
            millimeter!(10.0, 0.0, 10.0),
            degree!(0.0, 0.0, 0.0),
        )?))?;
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
            panic!("could not get LightData");
        }
        Ok(())
    }
    #[test]
    fn test_shifted_y() -> OpmResult<()> {
        let mut node = ParaxialSurface::new("test", millimeter!(10.))?;
        node.set_positioning(NodePositioning::Absolute(Isometry::new(
            millimeter!(0.0, 10.0, 10.0),
            degree!(0.0, 0.0, 0.0),
        )?))?;
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
            panic!("could not get LightData");
        }
        Ok(())
    }

    #[test]
    fn test_rotated_y() -> OpmResult<()> {
        let mut node = ParaxialSurface::new("test", millimeter!(10.))?;
        node.set_positioning(NodePositioning::Absolute(Isometry::new(
            millimeter!(0.0, 0.0, 10.0),
            degree!(45.0, 0.0, 0.0),
        )?))?;
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
            panic!("could not get LightData");
        }
        Ok(())
    }

    #[test]
    fn test_rotated_x() -> OpmResult<()> {
        let mut node = ParaxialSurface::new("test", millimeter!(10.))?;
        node.set_positioning(NodePositioning::Absolute(Isometry::new(
            millimeter!(0.0, 0.0, 10.0),
            degree!(0.0, 45.0, 0.0),
        )?))?;
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
            panic!("could not get LightData");
        }
        Ok(())
    }
}
