#![warn(missing_docs)]
//! Lens with spherical or flat surfaces
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    millimeter,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::Proptype,
    refractive_index::{refr_index_vaccuum, RefrIndexConst, RefractiveIndex, RefractiveIndexType},
    surface::{Plane, Sphere},
    utils::{geom_transformation::Isometry, EnumProxy},
};

use num::Zero;
use uom::si::f64::Length;

use super::node_attr::NodeAttr;

#[derive(Debug)]
/// A real lens with spherical (or flat) surfaces.
///
///
/// ## Optical Ports
///   - Inputs
///     - `front`
///   - Outputs
///     - `rear`
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `front curvature`
///   - `rear curvature`
///   - `center thickness`
///   - `refractive index`
pub struct Lens {
    node_attr: NodeAttr,
}

impl Default for Lens {
    /// Create a lens with a center thickness of 10.0 mm. front & back radii of curvature of 500.0 mm and a refractive index of 1.5.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("lens", "lens");
        node_attr
            .create_property(
                "front curvature",
                "radius of curvature of front surface",
                None,
                millimeter!(500.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "rear curvature",
                "radius of curvature of rear surface",
                None,
                millimeter!(-500.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "center thickness",
                "thickness of the lens in the center",
                None,
                millimeter!(10.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "refractive index",
                "refractive index of the lens material",
                None,
                EnumProxy::<RefractiveIndexType> {
                    value: RefractiveIndexType::Const(RefrIndexConst::new(1.5).unwrap()),
                }
                .into(),
            )
            .unwrap();

        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        node_attr.set_property("apertures", ports.into()).unwrap();
        Self { node_attr }
    }
}
impl Lens {
    /// Creates a new [`Lens`].
    ///
    /// This function creates a lens with spherical front and back surfaces, a given center thickness and refractive index.
    /// The radii of curvature must not be zero. The given refractive index must not be < 1.0. A radius of curvature of +/- infinity
    /// corresponds to a flat surface.
    ///
    /// # Errors
    ///
    /// This function return an error if the given parameters are not correct.
    pub fn new(
        name: &str,
        front_curvature: Length,
        rear_curvature: Length,
        center_thickness: Length,
        refractive_index: &dyn RefractiveIndex,
    ) -> OpmResult<Self> {
        let mut lens = Self::default();
        lens.node_attr.set_property("name", name.into())?;

        if front_curvature.is_zero() || front_curvature.is_nan() {
            return Err(OpossumError::Other(
                "front curvature must not be 0.0 or NaN".into(),
            ));
        }
        lens.node_attr
            .set_property("front curvature", front_curvature.into())?;
        if rear_curvature.is_zero() || rear_curvature.is_nan() {
            return Err(OpossumError::Other(
                "rear curvature must not be 0.0 or NaN".into(),
            ));
        }
        lens.node_attr
            .set_property("rear curvature", rear_curvature.into())?;
        if center_thickness.is_sign_negative() || !center_thickness.is_finite() {
            return Err(OpossumError::Other(
                "rear curvature must be >= 0.0 and finite".into(),
            ));
        }
        lens.node_attr
            .set_property("center thickness", center_thickness.into())?;

        lens.node_attr.set_property(
            "refractive index",
            EnumProxy::<RefractiveIndexType> {
                value: refractive_index.to_enum(),
            }
            .into(),
        )?;
        Ok(lens)
    }
}

impl Optical for Lens {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let Some(data) = incoming_data.get("front") else {
            return Ok(LightResult::default());
        };
        let light_data = match analyzer_type {
            AnalyzerType::Energy => data.clone(),
            AnalyzerType::RayTrace(_) => {
                if let LightData::Geometric(mut rays) = data.clone() {
                    let Ok(Proptype::Length(front_roc)) =
                        self.node_attr.get_property("front curvature")
                    else {
                        return Err(OpossumError::Analysis("cannot read front curvature".into()));
                    };
                    let Ok(Proptype::RefractiveIndex(index_model)) =
                        self.node_attr.get_property("refractive index")
                    else {
                        return Err(OpossumError::Analysis(
                            "cannot read refractive index".into(),
                        ));
                    };
                    let next_z_pos =
                        rays.absolute_z_of_last_surface() + rays.dist_to_next_surface();
                    let isometry = Isometry::new_along_z(next_z_pos)?;
                    if (*front_roc).is_infinite() {
                        let plane = Plane::new(&isometry);
                        rays.refract_on_surface(&plane, &index_model.value)?;
                    } else {
                        rays.refract_on_surface(
                            &Sphere::new(*front_roc, &isometry)?,
                            &index_model.value,
                        )?;
                    };

                    if let Some(aperture) = self.ports().input_aperture("front") {
                        rays.apodize(aperture)?;
                        if let AnalyzerType::RayTrace(config) = analyzer_type {
                            rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                        }
                    } else {
                        return Err(OpossumError::OpticPort("input aperture not found".into()));
                    };
                    let Ok(Proptype::Length(center_thickness)) =
                        self.node_attr.get_property("center thickness")
                    else {
                        return Err(OpossumError::Analysis(
                            "cannot read center thickness".into(),
                        ));
                    };
                    rays.set_dist_to_next_surface(*center_thickness);
                    let thickness_iso = Isometry::new_along_z(*center_thickness)?;
                    let Ok(Proptype::Length(rear_roc)) =
                        self.node_attr.get_property("rear curvature")
                    else {
                        return Err(OpossumError::Analysis("cannot read rear curvature".into()));
                    };
                    let isometry = isometry.append(&thickness_iso);
                    rays.set_refractive_index(&index_model.value)?;
                    if (*rear_roc).is_infinite() {
                        let plane = Plane::new(&isometry);
                        rays.refract_on_surface(&plane, &refr_index_vaccuum())?;
                    } else {
                        rays.refract_on_surface(
                            &Sphere::new(*rear_roc, &isometry)?,
                            &refr_index_vaccuum(),
                        )?;
                    };
                    if let Some(aperture) = self.ports().output_aperture("rear") {
                        rays.apodize(aperture)?;
                        if let AnalyzerType::RayTrace(config) = analyzer_type {
                            rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                        }
                    } else {
                        return Err(OpossumError::OpticPort("ouput aperture not found".into()));
                    };
                    LightData::Geometric(rays)
                } else {
                    return Err(OpossumError::Analysis(
                        "expected ray data at input port".into(),
                    ));
                }
            }
        };
        let light_result = LightResult::from([("rear".into(), light_data)]);
        Ok(light_result)
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.node_attr.set_property(name, prop)
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn set_isometry(&mut self, isometry: crate::utils::geom_transformation::Isometry) {
        self.node_attr.set_isometry(isometry);
    }
}

// impl SDF for Lens
// {
//     fn sdf_eval_point(&self, p: &nalgebra::Point3<f64>, p_out: &mut nalgebra::Point3<f64>) -> f64 {
//         self.isometry.inverse_transform_point_mut_f64(&p, p_out);
//         // (p.x * p.x + p.y * p.y + p.z * p.z).sqrt() - self.radius.value
//         (p_out.x.mul_add(p_out.x, p_out.y.mul_add(p_out.y, p_out.z*p_out.z)) ).sqrt() - self.radius.value
//     }
// }

impl Dottable for Lens {
    fn node_color(&self) -> &str {
        "aqua"
    }
}
#[cfg(test)]
mod test {
    use crate::{
        analyzer::RayTraceConfig, joule, millimeter, nanometer, nodes::test_helper::test_helper::*,
        position_distributions::Hexapolar, rays::Rays,
    };
    use nalgebra::Vector3;

    use super::*;
    #[test]
    fn default() {
        let node = Lens::default();
        assert_eq!(node.name(), "lens");
        assert_eq!(node.node_type(), "lens");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "aqua");
        assert!(node.as_group().is_err());
        let Ok(Proptype::Length(roc)) = node.node_attr.get_property("front curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(500.0));
        let Ok(Proptype::Length(roc)) = node.node_attr.get_property("rear curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(-500.0));
        let Ok(Proptype::Length(roc)) = node.node_attr.get_property("center thickness") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(10.0));
        let Ok(Proptype::RefractiveIndex(index)) = node.node_attr.get_property("refractive index")
        else {
            panic!()
        };
        assert_eq!(
            (*index).value.get_refractive_index(Length::zero()).unwrap(),
            1.5
        );
    }
    #[test]
    fn new() {
        let roc = millimeter!(100.0);
        let ct = millimeter!(11.0);
        let ref_index = RefrIndexConst::new(1.5).unwrap();

        assert!(Lens::new("test", roc, roc, millimeter!(-0.1), &ref_index).is_err());
        assert!(Lens::new("test", roc, roc, millimeter!(f64::NAN), &ref_index).is_err());
        assert!(Lens::new("test", roc, roc, millimeter!(f64::INFINITY), &ref_index).is_err());

        assert!(Lens::new("test", roc, Length::zero(), ct, &ref_index).is_err());
        assert!(Lens::new("test", roc, millimeter!(f64::NAN), ct, &ref_index).is_err());
        assert!(Lens::new("test", roc, millimeter!(f64::INFINITY), ct, &ref_index).is_ok());
        assert!(Lens::new("test", roc, millimeter!(f64::NEG_INFINITY), ct, &ref_index).is_ok());

        assert!(Lens::new("test", Length::zero(), roc, ct, &ref_index).is_err());
        assert!(Lens::new("test", millimeter!(f64::NAN), roc, ct, &ref_index).is_err());
        assert!(Lens::new("test", millimeter!(f64::INFINITY), roc, ct, &ref_index).is_ok());
        assert!(Lens::new("test", millimeter!(f64::NEG_INFINITY), roc, ct, &ref_index).is_ok());
        let ref_index = RefrIndexConst::new(2.0).unwrap();
        let node = Lens::new("test", roc, roc, ct, &ref_index).unwrap();
        assert_eq!(node.name(), "test");
        let Ok(Proptype::Length(roc)) = node.node_attr.get_property("front curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(100.0));
        let Ok(Proptype::Length(roc)) = node.node_attr.get_property("rear curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(100.0));
        let Ok(Proptype::Length(roc)) = node.node_attr.get_property("center thickness") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(11.0));
        let Ok(Proptype::RefractiveIndex(EnumProxy::<RefractiveIndexType> {
            value: RefractiveIndexType::Const(ref_index_const),
        })) = node.node_attr.get_property("refractive index")
        else {
            panic!()
        };
        assert_eq!(
            (*ref_index_const)
                .get_refractive_index(Length::zero())
                .unwrap(),
            2.0
        );
    }
    #[test]
    fn inverted() {
        test_inverted::<Lens>()
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<Lens>()
    }
    #[test]
    fn analyze_wrong_port() {
        let mut node = Lens::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("rear".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_flatflat() {
        let mut node = Lens::new(
            "test",
            millimeter!(f64::INFINITY),
            millimeter!(f64::NEG_INFINITY),
            millimeter!(10.0),
            &RefrIndexConst::new(2.0).unwrap(),
        )
        .unwrap();
        let mut rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(10.0), 3).unwrap(),
        )
        .unwrap();
        rays.set_dist_to_next_surface(millimeter!(10.0));
        let mut incoming_data = LightResult::default();
        incoming_data.insert("front".into(), LightData::Geometric(rays));
        let output = node
            .analyze(
                incoming_data,
                &AnalyzerType::RayTrace(RayTraceConfig::default()),
            )
            .unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("rear") {
            for ray in rays {
                assert_eq!(ray.direction(), Vector3::z());
                assert_eq!(ray.path_length(), millimeter!(30.0));
            }
        } else {
            assert!(false);
        }
    }
    #[test]
    fn analyze_biconvex() {
        // biconvex lens with index of 1.0 (="neutral" lens)
        let mut node = Lens::new(
            "test",
            millimeter!(100.0),
            millimeter!(-100.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0).unwrap(),
        )
        .unwrap();
        let mut rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(10.0), 3).unwrap(),
        )
        .unwrap();
        rays.set_dist_to_next_surface(millimeter!(10.0));
        let mut incoming_data = LightResult::default();
        incoming_data.insert("front".into(), LightData::Geometric(rays));
        let output = node
            .analyze(
                incoming_data,
                &AnalyzerType::RayTrace(RayTraceConfig::default()),
            )
            .unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("rear") {
            for ray in rays {
                assert_eq!(ray.direction(), Vector3::z());
            }
        } else {
            assert!(false);
        }
    }
}
