#![warn(missing_docs)]
//! Lens with spherical or flat surfaces
use std::collections::HashMap;

use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    millimeter,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
    refractive_index::{RefrIndexConst, RefractiveIndex, RefractiveIndexType},
    surface::{Plane, Sphere},
    utils::EnumProxy,
};
use num::Zero;
use uom::si::f64::Length;

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
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut props = Properties::new("lens", "lens");
    props
        .create(
            "front curvature",
            "radius of curvature of front surface",
            None,
            millimeter!(500.0).into(),
        )
        .unwrap();
    props
        .create(
            "rear curvature",
            "radius of curvature of rear surface",
            None,
            millimeter!(-500.0).into(),
        )
        .unwrap();
    props
        .create(
            "center thickness",
            "thickness of the lens in the center",
            None,
            millimeter!(10.0).into(),
        )
        .unwrap();
    props
        .create(
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
    props.set("apertures", ports.into()).unwrap();
    props
}
impl Default for Lens {
    /// Create a lens with a center thickness of 10.0 mm. front & back radii of curvature of 500.0 mm and a refractive index of 1.5.
    fn default() -> Self {
        Self {
            props: create_default_props(),
        }
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
        let mut props = create_default_props();
        props.set("name", name.into())?;

        if front_curvature.is_zero() || front_curvature.is_nan() {
            return Err(OpossumError::Other(
                "front curvature must not be 0.0 or NaN".into(),
            ));
        }
        props.set("front curvature", front_curvature.into())?;
        if rear_curvature.is_zero() || rear_curvature.is_nan() {
            return Err(OpossumError::Other(
                "rear curvature must not be 0.0 or NaN".into(),
            ));
        }
        props.set("rear curvature", rear_curvature.into())?;
        if center_thickness.is_sign_negative() || !center_thickness.is_finite() {
            return Err(OpossumError::Other(
                "rear curvature must be >= 0.0 and finite".into(),
            ));
        }
        props.set("center thickness", center_thickness.into())?;
        props.set(
            "refractive index",
            EnumProxy::<RefractiveIndexType> {
                value: refractive_index.to_enum(),
            }
            .into(),
        )?;
        Ok(Self { props })
    }
}

impl Optical for Lens {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        match analyzer_type {
            AnalyzerType::Energy => Err(OpossumError::Analysis(
                "Energy Analysis is not yet implemented for Lens Nodes".into(),
            )),
            AnalyzerType::RayTrace(_) => {
                let data = incoming_data.get("front").unwrap_or(&None);
                if let Some(LightData::Geometric(rays)) = data {
                    let mut rays = rays.clone();
                    let Ok(Proptype::Length(front_roc)) = self.props.get("front curvature") else {
                        return Err(OpossumError::Analysis("cannot read front curvature".into()));
                    };
                    let Ok(Proptype::RefractiveIndex(index_model)) =
                        self.props.get("refractive index")
                    else {
                        return Err(OpossumError::Analysis(
                            "cannot read refractive index".into(),
                        ));
                    };
                    let next_z_pos =
                        rays.absolute_z_of_last_surface() + rays.dist_to_next_surface();
                    if (*front_roc).is_infinite() {
                        rays.refract_on_surface(&Plane::new(next_z_pos)?, &index_model.value)?;
                    } else {
                        rays.refract_on_surface(
                            &Sphere::new(next_z_pos, *front_roc)?,
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
                    let Ok(Proptype::Length(center_thickness)) = self.props.get("center thickness")
                    else {
                        return Err(OpossumError::Analysis(
                            "cannot read center thickness".into(),
                        ));
                    };
                    rays.set_dist_to_next_surface(*center_thickness);
                    let Ok(Proptype::Length(rear_roc)) = self.props.get("rear curvature") else {
                        return Err(OpossumError::Analysis("cannot read rear curvature".into()));
                    };
                    let next_z_pos =
                        rays.absolute_z_of_last_surface() + rays.dist_to_next_surface();
                    rays.set_refractive_index(&index_model.value)?;
                    let index_1_0 = &RefractiveIndexType::Const(RefrIndexConst::new(1.0).unwrap());
                    if (*rear_roc).is_infinite() {
                        rays.refract_on_surface(&Plane::new(next_z_pos)?, index_1_0)?;
                    } else {
                        rays.refract_on_surface(&Sphere::new(next_z_pos, *rear_roc)?, index_1_0)?;
                    };
                    if let Some(aperture) = self.ports().output_aperture("rear") {
                        rays.apodize(aperture)?;
                        if let AnalyzerType::RayTrace(config) = analyzer_type {
                            rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                        }
                    } else {
                        return Err(OpossumError::OpticPort("ouput aperture not found".into()));
                    };
                    Ok(HashMap::from([(
                        "rear".into(),
                        Some(LightData::Geometric(rays)),
                    )]))
                } else {
                    Err(OpossumError::Analysis(
                        "expected ray data at input port".into(),
                    ))
                }
            }
        }
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.props.set(name, prop)
    }
}

impl Dottable for Lens {
    fn node_color(&self) -> &str {
        "aqua"
    }
}
#[cfg(test)]
mod test {
    use crate::{
        analyzer::RayTraceConfig, joule, millimeter, nanometer, position_distributions::Hexapolar,
        rays::Rays,
    };
    use nalgebra::Vector3;

    use super::*;
    #[test]
    fn default() {
        let node = Lens::default();
        assert_eq!(node.properties().name().unwrap(), "lens");
        assert_eq!(node.properties().node_type().unwrap(), "lens");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "aqua");
        assert!(node.as_group().is_err());
        let Ok(Proptype::Length(roc)) = node.props.get("front curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(500.0));
        let Ok(Proptype::Length(roc)) = node.props.get("rear curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(-500.0));
        let Ok(Proptype::Length(roc)) = node.props.get("center thickness") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(10.0));
        let Ok(Proptype::RefractiveIndex(index)) = node.props.get("refractive index") else {
            panic!()
        };
        assert_eq!((*index).value.get_refractive_index(Length::zero()), 1.5);
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
        assert_eq!(node.props.name().unwrap(), "test");
        let Ok(Proptype::Length(roc)) = node.props.get("front curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(100.0));
        let Ok(Proptype::Length(roc)) = node.props.get("rear curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(100.0));
        let Ok(Proptype::Length(roc)) = node.props.get("center thickness") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(11.0));
        let Ok(Proptype::RefractiveIndex(EnumProxy::<RefractiveIndexType> {
            value: RefractiveIndexType::Const(ref_index_const),
        })) = node.props.get("refractive index")
        else {
            panic!()
        };
        assert_eq!((*ref_index_const).get_refractive_index(Length::zero()), 2.0);
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
        let mut incoming_data = HashMap::default();
        incoming_data.insert("front".into(), Some(LightData::Geometric(rays)));
        let output = node
            .analyze(
                incoming_data,
                &AnalyzerType::RayTrace(RayTraceConfig::default()),
            )
            .unwrap();
        if let Some(Some(LightData::Geometric(rays))) = output.get("rear") {
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
        let mut incoming_data = HashMap::default();
        incoming_data.insert("front".into(), Some(LightData::Geometric(rays)));
        let output = node
            .analyze(
                incoming_data,
                &AnalyzerType::RayTrace(RayTraceConfig::default()),
            )
            .unwrap();
        if let Some(Some(LightData::Geometric(rays))) = output.get("rear") {
            for ray in rays {
                assert_eq!(ray.direction(), Vector3::z());
            }
        } else {
            assert!(false);
        }
    }
    #[test]
    fn analyze_wrong() {
        let mut node = Lens::default();
        let mut rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(10.0), 3).unwrap(),
        )
        .unwrap();
        rays.set_dist_to_next_surface(millimeter!(10.0));
        let mut incoming_data = HashMap::default();
        incoming_data.insert("rear".into(), Some(LightData::Geometric(rays.clone())));
        assert!(node
            .analyze(
                incoming_data,
                &AnalyzerType::RayTrace(RayTraceConfig::default())
            )
            .is_err());
        let mut incoming_data = HashMap::default();
        incoming_data.insert("front".into(), Some(LightData::Geometric(rays)));
        assert!(node.analyze(incoming_data, &AnalyzerType::Energy).is_err());
    }
}
