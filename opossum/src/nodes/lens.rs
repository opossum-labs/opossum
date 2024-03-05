#![warn(missing_docs)]
//! Lens with spherical or flat surfaces
use std::collections::HashMap;

use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
    surface::{Plane, Sphere},
};
use num::Zero;
use uom::si::{f64::Length, length::millimeter};

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
            Length::new::<millimeter>(500.0).into(),
        )
        .unwrap();
    props
        .create(
            "rear curvature",
            "radius of curvature of rear surface",
            None,
            Length::new::<millimeter>(-500.0).into(),
        )
        .unwrap();
    props
        .create(
            "center thickness",
            "thickness of the lens in the center",
            None,
            Length::new::<millimeter>(10.0).into(),
        )
        .unwrap();
    props
        .create(
            "refractive index",
            "refractive index of the lens material",
            None,
            1.5.into(),
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
    /// The radii of curvature must not be zero. The given refractive index must not be < 1.0. A radius of curvature of +/- infinity corresponds to a flat surface.
    ///
    /// # Errors
    ///
    /// This function return an error if the given parameters are not correct.
    pub fn new(
        front_curvature: Length,
        rear_curvature: Length,
        center_thickness: Length,
        refractive_index: f64,
    ) -> OpmResult<Self> {
        let mut props = create_default_props();
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
        if refractive_index < 1.0 || !refractive_index.is_finite() {
            return Err(OpossumError::Other(
                "refractive index must be >= 1.0 and finite".into(),
            ));
        }
        props.set("refractive index", refractive_index.into())?;
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
                    let Ok(Proptype::F64(n2)) = self.props.get("refractive index") else {
                        return Err(OpossumError::Analysis(
                            "cannot read refractive index".into(),
                        ));
                    };
                    let next_z_pos =
                        rays.absolute_z_of_last_surface() + rays.dist_to_next_surface();
                    if (*front_roc).is_infinite() {
                        rays.refract_on_surface(&Plane::new(next_z_pos)?, *n2)?;
                    } else {
                        rays.refract_on_surface(&Sphere::new(next_z_pos, *front_roc)?, *n2)?;
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
                    rays.set_refractive_index(*n2)?;
                    if (*rear_roc).is_infinite() {
                        rays.refract_on_surface(&Plane::new(next_z_pos)?, 1.0)?;
                    } else {
                        rays.refract_on_surface(&Sphere::new(next_z_pos, *rear_roc)?, 1.0)?;
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
        "blue"
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn default() {
        let node=Lens::default();
        assert_eq!(node.properties().name().unwrap(), "lens");
        assert_eq!(node.properties().node_type().unwrap(), "lens");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "blue");
        assert!(node.as_group().is_err());
    }
}