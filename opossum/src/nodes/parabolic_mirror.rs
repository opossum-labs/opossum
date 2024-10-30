use nalgebra::Point2;
use uom::si::f64::{Angle, Length};

use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
        Analyzable, RayTraceConfig,
    },
    coatings::CoatingType,
    degree,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    light_result::LightResult,
    lightdata::LightData,
    meter,
    optic_node::{Alignable, OpticNode, LIDT},
    optic_ports::{OpticPorts, PortType},
    properties::Proptype,
    surface::{OpticalSurface, Parabola},
    utils::geom_transformation::Isometry,
};

use super::NodeAttr;

#[derive(Debug, Clone)]
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
pub struct ParabolicMirror {
    node_attr: NodeAttr,
    surface: OpticalSurface,
}
impl Default for ParabolicMirror {
    /// Create a parabolic mirror with a focal length of 1 meter.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("parabolic mirror");
        node_attr
            .create_property("focal length", "focal length", None, meter!(-1.0).into())
            .unwrap();
        node_attr
            .create_property(
                "oap angle x",
                "off axis angle around local x axis",
                None,
                degree!(0.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "oap angle y",
                "off axis angle around local y axis",
                None,
                degree!(0.0).into(),
            )
            .unwrap();
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "input").unwrap();
        ports
            .set_coating(
                &PortType::Input,
                "input",
                &CoatingType::ConstantR { reflectivity: 1.0 },
            )
            .unwrap();
        ports.add(&PortType::Output, "reflected").unwrap();
        node_attr.set_ports(ports);

        Self {
            node_attr,
            surface: OpticalSurface::new(Box::new(
                Parabola::new(meter!(-1.0), &Isometry::identity()).unwrap(),
            )),
        }
    }
}
impl ParabolicMirror {
    /// Creates a new [`ParabolicMirror`] node.
    ///
    /// This function creates a infinitely thin parabolic mirror with a given focal length.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given focal length is zero or not finite.
    pub fn new(name: &str, focal_length: Length) -> OpmResult<Self> {
        if !focal_length.is_normal() {
            return Err(OpossumError::Other(
                "focal length must not be 0.0 and finite".into(),
            ));
        }
        let mut parabola = Self::default();
        parabola.node_attr.set_name(name);
        parabola
            .node_attr
            .set_property("focal length", focal_length.into())?;
        parabola.update_surfaces()?;
        Ok(parabola)
    }
    /// Returns / modifies a [`ParabolicMirror`] with given off-axis angles.
    ///
    /// The angles define the off axis angles around the local x and y axis of the node. The given angles denote the full
    /// angle between an incoming and a reflected beam. Effectively this introduces a decentering
    /// of the node during positioning in 3D space such that the desired angles are met.
    ///
    /// # Errors
    ///
    /// This function will return an error if the node properties cannot be set.
    pub fn with_oap_angles(mut self, angles: Point2<Angle>) -> OpmResult<Self> {
        self.set_property("oap angle x", angles[0].into())?;
        self.set_property("oap angle y", angles[1].into())?;
        self.update_surfaces()?;
        Ok(self)
    }
}
impl OpticNode for ParabolicMirror {
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        let Ok(Proptype::Length(focal_length)) = self.node_attr.get_property("focal length") else {
            return Err(OpossumError::Analysis("cannot read focal length".into()));
        };
        let Ok(Proptype::Angle(oap_angle_x)) = self.node_attr.get_property("oap angle x") else {
            return Err(OpossumError::Analysis(
                "cannot read off axis angle x".into(),
            ));
        };
        let Ok(Proptype::Angle(oap_angle_y)) = self.node_attr.get_property("oap angle y") else {
            return Err(OpossumError::Analysis(
                "cannot read off axis angle y".into(),
            ));
        };
        let mut parabola = Parabola::new(*focal_length, &Isometry::identity())?;
        parabola.set_off_axis_angles((*oap_angle_x, *oap_angle_y));
        self.surface = OpticalSurface::new(Box::new(parabola));
        Ok(())
    }
    fn get_surface_mut(&mut self, _surf_name: &str) -> &mut OpticalSurface {
        &mut self.surface
    }
}
impl Alignable for ParabolicMirror {}
impl Dottable for ParabolicMirror {
    fn node_color(&self) -> &str {
        "chocolate2"
    }
}
impl LIDT for ParabolicMirror {}
impl Analyzable for ParabolicMirror {}
impl AnalysisGhostFocus for ParabolicMirror {}
impl AnalysisEnergy for ParabolicMirror {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let (inport, outport) = if self.inverted() {
            ("reflected", "input")
        } else {
            ("input", "reflected")
        };
        let Some(data) = incoming_data.get(inport) else {
            return Ok(LightResult::default());
        };
        Ok(LightResult::from([(outport.into(), data.clone())]))
    }
}
impl AnalysisRayTrace for ParabolicMirror {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let (inport, outport) = if self.inverted() {
            ("reflected", "input")
        } else {
            ("input", "reflected")
        };
        let Some(data) = incoming_data.get(inport) else {
            return Ok(LightResult::default());
        };
        if let LightData::Geometric(mut rays) = data.clone() {
            let reflected = if let Some(iso) = self.effective_iso() {
                let coating = self
                    .node_attr()
                    .ports()
                    .coating(&PortType::Input, "input")
                    .unwrap()
                    .clone();
                let surface = self.get_surface_mut("");
                surface.set_isometry(&iso);
                surface.set_coating(coating);
                let mut reflected_rays = rays.refract_on_surface(surface, None)?;
                if let Some(aperture) = self.ports().aperture(&PortType::Input, inport) {
                    reflected_rays.apodize(aperture, &iso)?;
                    reflected_rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                    reflected_rays
                } else {
                    return Err(OpossumError::OpticPort("input aperture not found".into()));
                }
            } else {
                return Err(OpossumError::Analysis(
                    "no location for surface defined. Aborting".into(),
                ));
            };
            let light_data = LightData::Geometric(reflected);
            let light_result = LightResult::from([(outport.into(), light_data)]);
            Ok(light_result)
        } else {
            Err(OpossumError::Analysis(
                "expected ray data at input port".into(),
            ))
        }
    }

    fn calc_node_position(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        AnalysisRayTrace::analyze(self, incoming_data, config)
    }
}
