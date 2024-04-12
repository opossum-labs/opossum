#![warn(missing_docs)]
//! A paraxial surface (ideal lens)
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    millimeter,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::Proptype,
    refractive_index::refr_index_vaccuum,
    surface::Plane,
};
use uom::num_traits::Zero;
use uom::si::f64::Length;

use super::node_attr::NodeAttr;

/// Paraxial surface (=ideal lens)
///
/// This node models a (flat) paraxial surface with a given `focal leength`. This corresponds to an ideal lens which is aberration free
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
#[derive(Debug, Clone)]
pub struct ParaxialSurface {
    node_attr: NodeAttr,
}
impl Default for ParaxialSurface {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("paraxial surface", "paraxial");

        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        node_attr.set_property("apertures", ports.into()).unwrap();

        node_attr
            .create_property(
                "focal length",
                "focal length",
                None,
                millimeter!(1.0).into(),
            )
            .unwrap();
        Self { node_attr }
    }
}
impl ParaxialSurface {
    /// Create a new paraxial surface node of the given focal length.
    ///
    /// # Errors
    /// This function returns an error if
    ///  - the given `focal_length` is 0.0 or not finite.
    /// # Panics
    /// This function panics if
    /// - the input port name already exists. (Theoretically impossible at this point, as the [`OpticPorts`] are created just before in this function)
    /// - the output port name already exists. (Theoretically impossible at this point, as the [`OpticPorts`] are created just before in this function)
    /// - the property `apertures` can not be set.
    pub fn new(name: &str, focal_length: Length) -> OpmResult<Self> {
        if focal_length.is_zero() || !focal_length.is_normal() {
            return Err(OpossumError::Other("focal length must be finite".into()));
        }
        let mut parsurf = Self::default();
        parsurf.node_attr.set_property("name", name.into())?;
        parsurf
            .node_attr
            .set_property("focal length", focal_length.into())?;
        Ok(parsurf)
    }
}
impl Optical for ParaxialSurface {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (src, target) = if self.properties().inverted()? {
            ("rear", "front")
        } else {
            ("front", "rear")
        };
        let Some(data) = incoming_data.get(src) else {
            return Ok(LightResult::default());
        };
        let light_data = match analyzer_type {
            AnalyzerType::Energy => data.clone(),
            AnalyzerType::RayTrace(_config) => {
                if let LightData::Geometric(mut rays) = data.clone() {
                    let Ok(Proptype::Length(focal_length)) =
                        self.node_attr.get_property("focal length")
                    else {
                        return Err(OpossumError::Analysis("cannot read focal length".into()));
                    };
                    let z_position =
                        rays.absolute_z_of_last_surface() + rays.dist_to_next_surface();
                    let plane = Plane::new_along_z(z_position)?;
                    rays.refract_on_surface(&plane, &refr_index_vaccuum())?;
                    rays.refract_paraxial(*focal_length)?;
                    if let Some(aperture) = self.ports().input_aperture("front") {
                        rays.apodize(aperture)?;
                        if let AnalyzerType::RayTrace(config) = analyzer_type {
                            rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                        }
                    } else {
                        return Err(OpossumError::OpticPort("input aperture not found".into()));
                    };
                    if let Some(aperture) = self.ports().output_aperture("rear") {
                        rays.apodize(aperture)?;
                        if let AnalyzerType::RayTrace(config) = analyzer_type {
                            rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                        }
                    } else {
                        return Err(OpossumError::OpticPort("input aperture not found".into()));
                    };
                    LightData::Geometric(rays)
                } else {
                    return Err(crate::error::OpossumError::Analysis(
                        "No LightData::Geometric for analyzer type RayTrace".into(),
                    ));
                }
            }
        };
        let mut light_result = LightResult::default();
        light_result.insert(target.into(), light_data);
        Ok(light_result)
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.node_attr.set_property(name, prop)
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
}

impl Dottable for ParaxialSurface {
    fn node_color(&self) -> &str {
        "palegreen"
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{aperture::Aperture, millimeter, rays::Rays};
    use assert_matches::assert_matches;
    #[test]
    fn default() {
        let node = ParaxialSurface::default();
        assert_eq!(node.name(), "paraxial surface");
        assert_eq!(node.node_type(), "paraxial");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert!(node.properties().get("focal length").is_ok());
        assert_matches!(
            node.properties().get("focal length").unwrap(),
            Proptype::Length(_)
        );
        if let Ok(Proptype::Length(dist)) = node.properties().get("focal length") {
            assert_eq!(*dist, millimeter!(1.0));
        }
        assert_eq!(node.node_color(), "palegreen");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let node = ParaxialSurface::new("Test", millimeter!(1.0)).unwrap();
        assert_eq!(node.name(), "Test");
        if let Ok(Proptype::F64(dist)) = node.properties().get("focal length") {
            assert_eq!(dist, &1.0);
        }
        assert!(ParaxialSurface::new("Test", millimeter!(-1.0)).is_ok());
        assert!(ParaxialSurface::new("Test", millimeter!(0.0)).is_err());
        assert!(ParaxialSurface::new("Test", millimeter!(f64::NAN)).is_err());
        assert!(ParaxialSurface::new("Test", millimeter!(f64::INFINITY)).is_err());
        assert!(ParaxialSurface::new("Test", millimeter!(f64::NEG_INFINITY)).is_err());
    }
    #[test]
    fn name_property() {
        let mut node = ParaxialSurface::default();
        node.set_property("name", "Test1".into()).unwrap();
        assert_eq!(node.name(), "Test1")
    }
    #[test]
    fn node_type_readonly() {
        let mut node = ParaxialSurface::default();
        assert!(node.set_property("node_type", "other".into()).is_err());
    }
    #[test]
    fn inverted() {
        let mut node = ParaxialSurface::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.properties().inverted().unwrap(), true)
    }
    #[test]
    fn ports() {
        let node = ParaxialSurface::default();
        assert_eq!(node.ports().input_names(), vec!["front"]);
        assert_eq!(node.ports().output_names(), vec!["rear"]);
    }
    #[test]
    fn analyze_empty() {
        let mut node = ParaxialSurface::default();
        let output = node
            .analyze(LightResult::default(), &AnalyzerType::Energy)
            .unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_wrong_port() {
        let mut node = ParaxialSurface::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("rear".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn set_input_aperture() {
        let mut node = ParaxialSurface::default();
        let aperture = Aperture::default();
        assert!(node.set_input_aperture("front", &aperture).is_ok());
        assert!(node.set_input_aperture("rear", &aperture).is_err());
        assert!(node.set_input_aperture("no port", &aperture).is_err());
    }
    #[test]
    fn set_output_aperture() {
        let mut node = ParaxialSurface::default();
        let aperture = Aperture::default();
        assert!(node.set_output_aperture("rear", &aperture).is_ok());
        assert!(node.set_output_aperture("front", &aperture).is_err());
        assert!(node.set_output_aperture("no port", &aperture).is_err());
    }
    // #[test]
    // #[ignore]
    // fn export_data() {
    //     assert!(ParaxialSurface::default()
    //         .export_data(Path::new(""))
    //         .is_ok());
    // }
    #[test]
    fn as_ref_node_mut() {
        let mut node = ParaxialSurface::default();
        assert!(node.as_refnode_mut().is_err());
    }
}
