#![warn(missing_docs)]
//! Infinitely thin mirror with spherical or flat surface
use std::f64::consts::PI;

use super::NodeAttr;
use crate::{
    analyzers::{
        GhostFocusConfig, RayTraceConfig, energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus,
        propagation_strategy::MissedSurfaceStrategy, raytrace::AnalysisRayTrace,
    },
    error::{OpmResult, OpossumError},
    light_result::{LightRays, LightResult},
    lightdata::LightData,
    nodes::NodeRegistration,
    num_per_mm,
    optic_node::OpticNode,
    optic_ports::PortType,
    properties::{Proptype, validator::Validator},
    radian,
    rays::Rays,
    refractive_index::refr_index_vaccuum,
    utils::to_f64,
};
use nalgebra::Vector3;
use opm_macros_lib::OpmNode;
use uom::si::{
    angle::radian,
    f64::{Angle, Length},
    length::nanometer,
    linear_number_density::per_millimeter,
};

/// a type definition for a linear number density: `1/length_unit`.
/// used, for example, for the periodic grating structure
pub type LinearDensity = uom::si::f64::LinearNumberDensity;

inventory::submit! {
    NodeRegistration::new::<ReflectiveGrating>("reflective grating", "reflective optical grating")
}

#[derive(OpmNode, Debug, Clone)]
#[opm_node("cornsilk")]
/// An infinitely thin reflective grating.
///
///
/// ## Optical Ports
///   - Inputs
///     - `input`
///   - Outputs
///     - `diffracted`
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `line density`
pub struct ReflectiveGrating {
    node_attr: NodeAttr,
}
unsafe impl Send for ReflectiveGrating {}

impl Default for ReflectiveGrating {
    /// Create a reflective grating with a specified line density.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("reflective grating");
        node_attr
            .create_property_with_validator(
                "line density",
                "line density in 1/mm of this grating",
                Validator::AndValidator {
                    validators: vec![
                        Validator::NumericIsFinite,
                        Validator::NumericIsNotZero,
                        Validator::NumericIsPositive,
                    ],
                },
                Proptype::LinearDensity(num_per_mm!(1740.)),
            )
            .unwrap();
        node_attr
            .create_property(
                "diffraction order",
                "order of diffraction that should be used to propagate the rays",
                (-1).into(),
            )
            .unwrap();
        let mut g = Self { node_attr };
        g.update_surfaces().unwrap();
        g
    }
}
impl ReflectiveGrating {
    /// Creates a new [`ReflectiveGrating`].
    ///
    /// This function creates a reflective grating with a specified line-density on a flat surface.
    /// The grating vector (direction along the periodicty) is allways applied in x direction in the origin.
    /// # Errors
    /// This function errors if the properties `line_density` or `diffraction_order` can not be set or if the line density is negative or non finite
    pub fn new(name: &str, line_density: LinearDensity, diffraction_order: i32) -> OpmResult<Self> {
        let mut grating = Self::default();
        grating.node_attr.set_name(name);
        grating
            .node_attr
            .set_property("line density", Proptype::LinearDensity(line_density))?;
        grating
            .node_attr
            .set_property("diffraction order", diffraction_order.into())?;
        Ok(grating)
    }

    /// Set the angle of a grating such that the incoming ray has an angle of `angle` to littrow
    /// # Errors
    /// This function errors if
    /// - the diffraction order cannot be read from the properties
    /// - the line density cannot be read from the properties
    pub fn with_rot_from_littrow(self, wavelength: Length, angle: Angle) -> OpmResult<Self> {
        let Ok(Proptype::I32(diffraction_order)) = self.node_attr.get_property("diffraction order")
        else {
            return Err(OpossumError::Analysis(
                "cannot read diffraction order".into(),
            ));
        };
        let Ok(Proptype::LinearDensity(line_density)) = self.node_attr.get_property("line density")
        else {
            return Err(OpossumError::Analysis("cannot read line density".into()));
        };
        let x = to_f64(*diffraction_order) * wavelength.value * line_density.value / 2.;

        if x.abs() > 1.0 {
            return Err(OpossumError::Analysis(format!(
                "Wavelength {} nm is too large for grating constant {} lines/mm and order {} (evanescent waves)",
                wavelength.get::<nanometer>(),
                line_density.get::<per_millimeter>(),
                diffraction_order
            )));
        }
        let littrow = x.asin();
        self.with_tilt(radian!(0., littrow + angle.get::<radian>(), 0.0))
    }
    /// Set the angle of a grating such that the outgoing ray has an angle of `angle` to littrow
    /// # Errors
    /// This function errors if
    /// - the diffraction order cannot be read from the properties
    /// - the line density cannot be read from the properties
    pub fn to_rot_from_littrow(self, wavelength: Length, angle: Angle) -> OpmResult<Self> {
        let Ok(Proptype::I32(diffraction_order)) = self.node_attr.get_property("diffraction order")
        else {
            return Err(OpossumError::Analysis(
                "cannot read diffraction order".into(),
            ));
        };
        let Ok(Proptype::LinearDensity(line_density)) = self.node_attr.get_property("line density")
        else {
            return Err(OpossumError::Analysis("cannot read line density".into()));
        };
        let littrow =
            (to_f64(*diffraction_order) * wavelength.value * line_density.value / 2.).asin();
        let angle_in_rad = angle.get::<radian>();
        let rot_angle = (to_f64(*diffraction_order) * wavelength.value)
            .mul_add(line_density.value, -(littrow + angle_in_rad).sin())
            .asin();
        self.with_tilt(radian!(0.0, rot_angle, 0.0))
    }
}
impl AnalysisGhostFocus for ReflectiveGrating {
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
impl AnalysisEnergy for ReflectiveGrating {}
impl AnalysisRayTrace for ReflectiveGrating {
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
            let Proptype::I32(diffraction_order) =
                self.node_attr.get_property("diffraction order")?.clone()
            else {
                return Err(OpossumError::Analysis(
                    "cannot read diffraction order".into(),
                ));
            };
            let Proptype::LinearDensity(line_density) =
                self.node_attr.get_property("line density")?.clone()
            else {
                return Err(OpossumError::Analysis("cannot read line density".into()));
            };

            let iso = self.effective_surface_iso(in_port)?;
            if let Some(surf) = self.get_optic_surface_mut(in_port) {
                let refraction_intended = false;
                let grating_vector =
                    2. * PI * line_density.value * iso.transform_vector_f64(&Vector3::x());
                let mut diffracted_rays = rays.diffract_on_periodic_surface(
                    surf,
                    &refr_index_vaccuum(),
                    grating_vector,
                    &diffraction_order,
                    refraction_intended,
                )?;
                match self.ports().aperture(&PortType::Input, in_port) {
                    Some(aperture) => {
                        diffracted_rays.apodize(aperture, &iso)?;
                        diffracted_rays
                            .invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                    }
                    _ => {
                        return Err(OpossumError::OpticPort("input aperture not found".into()));
                    }
                }

                let light_result =
                    LightResult::from([(out_port.into(), LightData::Geometric(diffracted_rays))]);
                Ok(light_result)
            } else {
                Err(OpossumError::Analysis("no surface found. Aborting".into()))
            }
        } else {
            Err(OpossumError::Analysis(
                "expected ray data at input port".into(),
            ))
        }
    }
}

impl OpticNode for ReflectiveGrating {
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{RayTraceConfig, energy::EnergyConfig},
        degree, joule, millimeter, nanometer,
        nodes::test_helper::test_helper::*,
        optic_ports::PortType,
        ray::Ray,
        rays::Rays,
        spectrum_helper::create_he_ne_spec,
        utils::geom_transformation::Isometry,
    };
    use approx::assert_relative_eq;
    use core::f64;
    use nalgebra::vector;
    #[test]
    fn default() {
        let node = ReflectiveGrating::default();
        assert_eq!(node.name(), "reflective grating");
        assert_eq!(node.node_type(), "reflective grating");
        assert_eq!(node.node_color(), "cornsilk");
        assert_eq!(node.inverted(), false);
        if let Ok(Proptype::I32(order)) = node.properties().get("diffraction order") {
            assert_eq!(*order, -1);
        } else {
            assert!(false, "property diffraction order was not an I32.");
        }
        if let Ok(Proptype::LinearDensity(line_density)) = node.properties().get("line density") {
            assert_eq!(*line_density, num_per_mm!(1740.));
        } else {
            assert!(false, "property line density was not a LinearDensity.");
        }
    }
    #[test]
    fn new() {
        let node = ReflectiveGrating::new("test", num_per_mm!(200.), 1).unwrap();
        assert_eq!(node.name(), "test");
        assert_eq!(node.node_type(), "reflective grating");
        if let Ok(Proptype::I32(order)) = node.properties().get("diffraction order") {
            assert_eq!(*order, 1);
        } else {
            assert!(false, "property diffraction order was not an I32.");
        }
        if let Ok(Proptype::LinearDensity(line_density)) = node.properties().get("line density") {
            assert_eq!(*line_density, num_per_mm!(200.));
        } else {
            assert!(false, "property line density was not a LinearDensity.");
        }
    }
    #[test]
    fn with_rot_from_littrow_math() {
        let line_density = num_per_mm!(1000.0);
        let diffraction_order = 1;
        let wavelength = nanometer!(500.0);

        let node = ReflectiveGrating::new("test_grating", line_density, diffraction_order)
            .unwrap()
            .with_rot_from_littrow(wavelength, degree!(0.0))
            .unwrap();

        // Manual calculation: littrow = asin(m * lambda * G / 2)
        // 1 * 500e-6 mm * 1000 l/mm / 2 = 0.25
        let expected_littrow = (1.0_f64 * 500e-6 * 1000.0 / 2.0).asin();
        let actual_tilt = node
            .node_attr()
            .alignment()
            .as_ref()
            .expect("Alignment should be set by with_tilt")
            .rotation()[1]
            .get::<radian>();

        assert_relative_eq!(actual_tilt, expected_littrow, epsilon = 1e-12);
    }

    #[test]
    fn with_rot_from_littrow_with_offset() {
        let line_density = num_per_mm!(1200.0);
        let diffraction_order = -1;
        let wavelength = nanometer!(632.8);
        let offset = degree!(5.0);

        let node = ReflectiveGrating::new("test_offset", line_density, diffraction_order)
            .unwrap()
            .with_rot_from_littrow(wavelength, offset)
            .unwrap();

        // littrow = asin(-1 * 632.8e-6 * 1200.0 / 2.0)
        let littrow = (-1.0_f64 * 632.8e-6 * 1200.0 / 2.0).asin();
        let expected_tilt = littrow + offset.get::<radian>();
        let actual_tilt = node
            .node_attr()
            .alignment()
            .as_ref()
            .expect("Alignment should be set by with_tilt")
            .rotation()[1]
            .get::<radian>();

        assert_relative_eq!(actual_tilt, expected_tilt, epsilon = 1e-12);
    }
    #[test]
    fn with_rot_from_littrow_impossible_physics() {
        let line_density = num_per_mm!(5000.0);
        let wavelength = nanometer!(1000.0);
        let diffraction_order = 1;

        // x = (1 * 1e-3 mm * 5000 mm^-1) / 2 = 2.5
        // asin(2.5) is NaN!
        let node = ReflectiveGrating::new("test", line_density, diffraction_order).unwrap();
        let result = node.with_rot_from_littrow(wavelength, degree!(0.0));

        let err_msg = result.unwrap_err();
        assert_eq!(err_msg, OpossumError::Analysis("Wavelength 1000 nm is too large for grating constant 5000 lines/mm and order 1 (evanescent waves)".into()));
    }
    #[test]
    fn invalid_line_density() {
        assert!(ReflectiveGrating::new("test", num_per_mm!(200.), 1).is_ok());
        assert!(ReflectiveGrating::new("test", num_per_mm!(-200.), 1).is_err());
        assert!(ReflectiveGrating::new("test", num_per_mm!(0.), 1).is_err());
        assert!(ReflectiveGrating::new("test", num_per_mm!(f64::NEG_INFINITY), 1).is_err());
        assert!(ReflectiveGrating::new("test", num_per_mm!(f64::INFINITY), 1).is_err());
        assert!(ReflectiveGrating::new("test", num_per_mm!(f64::NAN), 1).is_err());
    }
    #[test]
    fn ports() {
        let node = ReflectiveGrating::default();
        assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn set_aperture() {
        test_set_aperture::<ReflectiveGrating>("input_1", "output_1");
    }
    #[test]
    fn inverted() {
        test_inverted::<ReflectiveGrating>()
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<ReflectiveGrating>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = ReflectiveGrating::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default()).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_energy_ok() {
        let mut node = ReflectiveGrating::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default()).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_geometric_wrong_data_type() {
        test_analyze_wrong_data_type::<ReflectiveGrating>("input_1");
    }
    #[test]
    fn analyze_geometric_no_isometery() {
        test_analyze_geometric_no_isometry::<ReflectiveGrating>("input_1");
    }
    #[test]
    fn analyze_geometric_littrow_ok() {
        let mut node = ReflectiveGrating::default()
            .with_rot_from_littrow(nanometer!(1000.), degree!(0.))
            .unwrap();
        node.set_isometry(Isometry::new(millimeter!(0., 0., 0.), degree!(0., 0., 0.)).unwrap())
            .unwrap();
        let mut input = LightResult::default();
        let rays = Rays::from(Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap());
        let input_light = LightData::Geometric(rays);
        input.insert("input_1".into(), input_light.clone());
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.position(), millimeter!(0.0, 0.0, 0.0));

            let dir = vector![0.0, 0.0, -1.];
            assert_relative_eq!(ray.direction(), dir, epsilon = 1e-15);
        } else {
            assert!(false, "could not get LightData");
        }
    }

    #[test]
    fn analyze_geometric_1deg_from_littrow_ok() {
        let wvl = nanometer!(1000.);
        let angle_from_littrow = degree!(1.);
        let mut node = ReflectiveGrating::default()
            .with_rot_from_littrow(wvl, angle_from_littrow)
            .unwrap();
        node.set_isometry(Isometry::new(millimeter!(0., 0., 0.), degree!(0., 0., 0.)).unwrap())
            .unwrap();
        let mut input = LightResult::default();
        let rays = Rays::from(Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap());
        let input_light = LightData::Geometric(rays);
        input.insert("input_1".into(), input_light.clone());
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.position(), millimeter!(0.0, 0.0, 0.0));
            let input_angle = (-wvl.value * 1740000. / 2.).asin() + angle_from_littrow.value;
            let diffraction_angle = (-1740000. * wvl.value - input_angle.sin()).asin();
            let z_dir = (-input_angle + diffraction_angle).cos();
            let x_dir = (-input_angle + diffraction_angle).sin();
            let dir = vector![x_dir, 0.0, -z_dir];
            assert_relative_eq!(ray.direction(), dir, epsilon = 1e-15);
        } else {
            assert!(false, "could not get LightData");
        }
    }
}
