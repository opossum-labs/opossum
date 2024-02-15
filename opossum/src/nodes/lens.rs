//! Spherical lens
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
};
use num::Zero;
use uom::si::{f64::Length, length::millimeter};

#[derive(Debug)]
pub struct Lens {
    props: Properties,
    z_pos: Length,
}
fn create_default_props() -> Properties {
    let mut props = Properties::new("lens", "lens");
    props.create("front_curvature", "radius of curvature of front surface", None, Length::new::<millimeter>(500.0).into()).unwrap();
    props.create("rear_curvature", "radius of curvature of rear surface", None, Length::new::<millimeter>(-500.0).into()).unwrap();
    props.create("center thickness", "thickness of the lens in the center", None, Length::new::<millimeter>(10.0).into()).unwrap();
    props.create("refractive index", "refractive index of the lens material", None, 1.5.into()).unwrap();
    
    let mut ports = OpticPorts::new();
    ports.create_input("in1").unwrap();
    ports.create_output("out1").unwrap();
    props.set("apertures", ports.into()).unwrap();
    props
}
impl Default for Lens {
    /// Create a 100mm focal lengths lens. LA1251-B from thorlabs. refractive inde hardcoded for n-bk7 at 1054 nm
    fn default() -> Self {
        Self {
            props: create_default_props(),
            z_pos: Length::zero()
        }
    }
}
impl Lens {
    #[must_use]
    pub fn new(
        front_curvature: Length,
        rear_curvature: Length,
        center_thickness: Length,
        refractive_index: f64,
    ) -> Self {
        let mut props=create_default_props();
        props.set("front curvature", front_curvature.into()).unwrap();
        props.set("rear curvature", rear_curvature.into()).unwrap();
        props.set("center thickness", center_thickness.into()).unwrap();
        props.set("refractive index", refractive_index.into()).unwrap();
        Self {
            props,
            z_pos: Length::zero()
        }
    }
    #[must_use]
    pub fn position(&self) -> Length {
        self.z_pos
    }
}


impl Optical for Lens {
    fn analyze(
        &mut self,
        _incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        match analyzer_type {
            AnalyzerType::Energy => Err(OpossumError::Analysis(
                "Energy Analysis is not yet implemented for Lens Nodes".into(),
            )),
            _ => Err(OpossumError::Analysis(
                "No Analysis is currently implemented for Lens Nodes".into(),
            )),
            // AnalyzerType::RayTrace(_) => Ok(self.analyze_ray_trace(incoming_data)),
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
