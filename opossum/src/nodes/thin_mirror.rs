#![warn(missing_docs)]
//! Infinitely thin mirror with spherical or flat surface
use super::NodeAttr;
use crate::{
    analyzer::AnalyzerType,
    coatings::CoatingType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    millimeter,
    optic_ports::OpticPorts,
    optical::{Alignable, LightResult, Optical},
    properties::Proptype,
    refractive_index::refr_index_vaccuum,
    surface::{OpticalSurface, Plane, Sphere},
};
use num::Zero;
use uom::si::f64::Length;

#[derive(Debug)]
/// An infinitely thin mirror with a spherical (or flat) surface.
///
///
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
        let mut ports = OpticPorts::new();
        ports.create_input("input").unwrap();
        ports
            .set_input_coating("input", &CoatingType::ConstantR { reflectivity: 1.0 })
            .unwrap();
        ports.create_output("reflected").unwrap();
        node_attr.set_ports(ports);
        Self { node_attr }
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
        Ok(self)
    }
}
impl Optical for ThinMirror {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (inport, outport) = if self.inverted() {
            ("reflected", "input")
        } else {
            ("input", "reflected")
        };
        let Some(data) = incoming_data.get(inport) else {
            return Ok(LightResult::default());
        };
        let light_data = match analyzer_type {
            AnalyzerType::Energy => data.clone(),
            AnalyzerType::RayTrace(_) => {
                if let LightData::Geometric(mut rays) = data.clone() {
                    let Ok(Proptype::Length(roc)) = self.node_attr.get_property("curvature") else {
                        return Err(OpossumError::Analysis("curvature".into()));
                    };
                    let reflected = if let Some(iso) = self.effective_iso() {
                        let mut surface = if roc.is_infinite() {
                            OpticalSurface::new(Box::new(Plane::new(&iso)))
                        } else {
                            OpticalSurface::new(Box::new(Sphere::new(*roc, &iso)?))
                        };
                        surface.set_coating(
                            self.node_attr()
                                .ports()
                                .input_coating("input")
                                .unwrap()
                                .clone(),
                        );
                        let mut reflected_rays =
                            rays.refract_on_surface(&surface, &refr_index_vaccuum())?;
                        if let Some(aperture) = self.ports().input_aperture("input") {
                            reflected_rays.apodize(aperture)?;
                            if let AnalyzerType::RayTrace(config) = analyzer_type {
                                reflected_rays
                                    .invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                            }
                            reflected_rays
                        } else {
                            return Err(OpossumError::OpticPort("input aperture not found".into()));
                        }
                    } else {
                        return Err(OpossumError::Analysis(
                            "no location for surface defined. Aborting".into(),
                        ));
                    };
                    LightData::Geometric(reflected)
                } else {
                    return Err(OpossumError::Analysis(
                        "expected ray data at input port".into(),
                    ));
                }
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
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzer::RayTraceConfig, degree, joule, lightdata::DataEnergy, nanometer,
        nodes::test_helper::test_helper::*, ray::Ray, rays::Rays,
        spectrum_helper::create_he_ne_spec, utils::geom_transformation::Isometry,
    };
    use nalgebra::vector;
    #[test]
    fn default() {
        let node = ThinMirror::default();
        assert_eq!(node.name(), "mirror");
        assert_eq!(node.node_type(), "mirror");
        assert_eq!(node.is_detector(), false);
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
        assert_eq!(node.ports().input_names(), vec!["input"]);
        assert_eq!(node.ports().output_names(), vec!["reflected"]);
    }
    #[test]
    fn set_aperture() {
        test_set_aperture::<ThinMirror>("input", "reflected");
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
        input.insert("reflected".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_energy_ok() {
        let mut node = ThinMirror::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("input".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("reflected"));
        assert_eq!(output.len(), 1);
        let output = output.get("reflected");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_geometric_wrong_data_type() {
        test_analyze_wrong_data_type::<ThinMirror>("input");
    }
    #[test]
    fn analyze_geometric_no_isometery() {
        test_analyze_geometric_no_isometry::<ThinMirror>("input");
    }
    #[test]
    fn analyze_geometric_ok() {
        let mut node = ThinMirror::default();
        node.set_isometry(
            Isometry::new(millimeter!(0.0, 0.0, 10.0), degree!(0.0, 0.0, 0.0)).unwrap(),
        );
        let mut input = LightResult::default();
        let mut rays = Rays::default();
        rays.add_ray(Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap());
        let input_light = LightData::Geometric(rays);
        input.insert("input".into(), input_light.clone());
        let output = node
            .analyze(input, &AnalyzerType::RayTrace(RayTraceConfig::default()))
            .unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("reflected") {
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
