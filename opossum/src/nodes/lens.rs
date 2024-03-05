//! Spherical lens
use std::collections::HashMap;

use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
    surface::Sphere,
};
use uom::si::{f64::Length, length::millimeter};

#[derive(Debug)]
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
    /// The radii of curvature must not be zero. The given refractive index must not be < 1.0.
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
        props.set("front curvature", front_curvature.into())?;
        props.set("rear curvature", rear_curvature.into())?;
        props.set("center thickness", center_thickness.into())?;
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
                    let front_surface = Sphere::new(next_z_pos, *front_roc)?;
                    rays.refract_on_surface(&front_surface, *n2)?;
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
                    let rear_surface = Sphere::new(next_z_pos, *rear_roc)?;
                    rays.refract_on_surface(&rear_surface, 1.0)?;
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
