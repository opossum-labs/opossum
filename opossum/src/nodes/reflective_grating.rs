#![warn(missing_docs)]
//! Infinitely thin mirror with spherical or flat surface
use std::f64::consts::PI;

use super::NodeAttr;
use crate::{
    analyzers::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    num_per_mm,
    optic_ports::OpticPorts,
    optical::{Alignable, LightResult, Optical},
    properties::Proptype,
    radian,
    refractive_index::refr_index_vaccuum,
    surface::Plane,
};
use approx::relative_eq;
use nalgebra::Vector3;
use num::ToPrimitive;
use uom::si::{
    angle::radian,
    f64::{Angle, Length},
};

/// a type definition for a linear number density: `1/length_unit`.
/// used, for example, for the periodic grating structure
pub type LinearDensity = uom::si::f64::LinearNumberDensity;

#[derive(Debug, Clone)]
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
impl Default for ReflectiveGrating {
    /// Create a reflective grating with a specified line density.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("reflective grating");
        node_attr
            .create_property(
                "line density",
                "line density in 1/mm of this grating",
                None,
                Proptype::LinearDensity(num_per_mm!(1740.)),
            )
            .unwrap();
        node_attr
            .create_property(
                "diffraction order",
                "order of diffraction that should be used to propagate the rays",
                None,
                (-1).into(),
            )
            .unwrap();
        let mut ports = OpticPorts::new();
        ports.create_input("input").unwrap();
        ports.create_output("diffracted").unwrap();
        node_attr.set_ports(ports);
        Self { node_attr }
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
        if !(line_density.value.is_finite()
            && line_density.value.is_sign_positive()
            && !relative_eq!(line_density.value, 0.))
        {
            return Err(OpossumError::Other(
                "Only positive finite values are allowed for a grating line density".into(),
            ));
        }
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
    /// - the diffraction order cannot be read from te properties
    /// - the line density cannot be read from te properties
    /// # Panics
    /// This function panics if the diffraction order canno be converted to f64
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
        let littrow = (diffraction_order.to_f64().unwrap() * wavelength.value * line_density.value
            / 2.)
            .asin();
        self.with_tilt(radian!(0., littrow + angle.get::<radian>(), 0.0))
    }
    /// Set the angle of a grating such that the outgoing ray has an angle of `angle` to littrow
    /// # Errors
    /// This function errors if
    /// - the diffraction order cannot be read from te properties
    /// - the line density cannot be read from te properties
    /// # Panics
    /// This function panics if the diffraction order canno be converted to f64
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
        let littrow = (diffraction_order.to_f64().unwrap() * wavelength.value * line_density.value
            / 2.)
            .asin();
        let angle_in_rad = angle.get::<radian>();
        let rot_angle = (diffraction_order.to_f64().unwrap() * wavelength.value)
            .mul_add(line_density.value, -(littrow + angle_in_rad).sin())
            .asin();
        self.with_tilt(radian!(0., rot_angle, 0.0))
    }
}
impl Alignable for ReflectiveGrating {}
impl Dottable for ReflectiveGrating {
    fn node_color(&self) -> &str {
        "cornsilk"
    }
}

impl Optical for ReflectiveGrating {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (inport, outport) = if self.inverted() {
            ("diffracted", "input")
        } else {
            ("input", "diffracted")
        };
        let Some(data) = incoming_data.get(inport) else {
            return Ok(LightResult::default());
        };
        let light_data = match analyzer_type {
            AnalyzerType::Energy => data.clone(),
            AnalyzerType::RayTrace(_) => {
                if let LightData::Geometric(mut rays) = data.clone() {
                    let Ok(Proptype::I32(diffraction_order)) =
                        self.node_attr.get_property("diffraction order")
                    else {
                        return Err(OpossumError::Analysis(
                            "cannot read diffraction order".into(),
                        ));
                    };
                    let Ok(Proptype::LinearDensity(line_density)) =
                        self.node_attr.get_property("line density")
                    else {
                        return Err(OpossumError::Analysis("cannot read line density".into()));
                    };

                    let diffracted = if let Some(iso) = self.effective_iso() {
                        let grating_vector =
                            2. * PI * line_density.value * iso.transform_vector_f64(&Vector3::x());
                        let _ = rays.diffract_on_periodic_surface(
                            &Plane::new(&iso),
                            &refr_index_vaccuum(),
                            grating_vector,
                            diffraction_order,
                        )?;

                        if let Some(aperture) = self.ports().input_aperture("input") {
                            rays.apodize(aperture)?;
                            if let AnalyzerType::RayTrace(config) = analyzer_type {
                                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                            }
                            rays
                        } else {
                            return Err(OpossumError::OpticPort("input aperture not found".into()));
                        }
                    } else {
                        return Err(OpossumError::Analysis(
                            "no location for surface defined. Aborting".into(),
                        ));
                    };
                    LightData::Geometric(diffracted)
                } else {
                    return Err(OpossumError::Analysis(
                        "expected ray data at input port".into(),
                    ));
                }
            }
            _ => {
                return Err(OpossumError::Analysis(
                    "analysis mode not yet implemented for reflective grating".into(),
                ))
            }
        };
        let light_result = LightResult::from([(outport.into(), light_data)]);
        Ok(light_result)
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
    use core::f64;

    use super::*;
    use crate::{
        analyzers::RayTraceConfig, degree, joule, lightdata::DataEnergy, millimeter, nanometer,
        nodes::test_helper::test_helper::*, ray::Ray, rays::Rays,
        spectrum_helper::create_he_ne_spec, utils::geom_transformation::Isometry,
    };
    use approx::assert_relative_eq;
    use nalgebra::vector;
    #[test]
    fn default() {
        let node = ReflectiveGrating::default();
        assert_eq!(node.name(), "reflective grating");
        assert_eq!(node.node_type(), "reflective grating");
        assert_eq!(node.is_detector(), false);
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
        assert_eq!(node.ports().input_names(), vec!["input"]);
        assert_eq!(node.ports().output_names(), vec!["diffracted"]);
    }
    #[test]
    fn set_aperture() {
        test_set_aperture::<ReflectiveGrating>("input", "diffracted");
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
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("diffracted".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_energy_ok() {
        let mut node = ReflectiveGrating::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("input".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("diffracted"));
        assert_eq!(output.len(), 1);
        let output = output.get("diffracted");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_geometric_wrong_data_type() {
        test_analyze_wrong_data_type::<ReflectiveGrating>("input");
    }
    #[test]
    fn analyze_geometric_no_isometery() {
        test_analyze_geometric_no_isometry::<ReflectiveGrating>("input");
    }
    #[test]
    fn analyze_geometric_littrow_ok() {
        let mut node = ReflectiveGrating::default()
            .with_rot_from_littrow(nanometer!(1000.), degree!(0.))
            .unwrap();
        node.set_isometry(Isometry::new(millimeter!(0., 0., 0.), degree!(0., 0., 0.)).unwrap());
        let mut input = LightResult::default();
        let mut rays = Rays::default();
        rays.add_ray(Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap());
        let input_light = LightData::Geometric(rays);
        input.insert("input".into(), input_light.clone());
        let output = node
            .analyze(input, &AnalyzerType::RayTrace(RayTraceConfig::default()))
            .unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("diffracted") {
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
        node.set_isometry(Isometry::new(millimeter!(0., 0., 0.), degree!(0., 0., 0.)).unwrap());
        let mut input = LightResult::default();
        let mut rays = Rays::default();
        rays.add_ray(Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap());
        let input_light = LightData::Geometric(rays);
        input.insert("input".into(), input_light.clone());
        let output = node
            .analyze(input, &AnalyzerType::RayTrace(RayTraceConfig::default()))
            .unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("diffracted") {
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
