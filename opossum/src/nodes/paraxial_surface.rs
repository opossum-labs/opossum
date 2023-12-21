#![warn(missing_docs)]
//! A paraxial surface (ideal lens)
use std::collections::HashMap;

use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
};
use uom::num_traits::Zero;
use uom::si::{f64::Length, length::millimeter};
fn create_default_props() -> Properties {
    let mut ports = OpticPorts::new();
    ports.create_input("front").unwrap();
    ports.create_output("rear").unwrap();
    let mut props = Properties::new("paraxial surface", "paraxial");
    props
        .create("focal length", "focal length in mm", None, 1.0.into())
        .unwrap();
    props.set("apertures", ports.into()).unwrap();
    props
}
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
    props: Properties,
}
impl Default for ParaxialSurface {
    fn default() -> Self {
        Self {
            props: create_default_props(),
        }
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
        let mut props = create_default_props();
        props.set("name", name.into())?;
        props.set("focal length", focal_length.get::<millimeter>().into())?;
        Ok(Self { props })
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
        let mut data = incoming_data.get(src).unwrap_or(&None).clone();
        match analyzer_type {
            AnalyzerType::Energy => (),
            AnalyzerType::RayTrace(_config) => {
                if let Some(LightData::Geometric(mut rays)) = data {
                    let focal_length =
                        if let Ok(Proptype::F64(length)) = self.props.get("focal length") {
                            Length::new::<millimeter>(*length)
                        } else {
                            return Err(OpossumError::Analysis("cannot read focal length".into()));
                        };
                    rays.refract_paraxial(focal_length)?;
                    data = Some(LightData::Geometric(rays));
                } else {
                    return Err(crate::error::OpossumError::Analysis(
                        "No LightData::Geometric for analyzer type RayTrace".into(),
                    ));
                }
            }
        }
        Ok(HashMap::from([(target.into(), data)]))
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.props.set(name, prop)
    }
}

impl Dottable for ParaxialSurface {
    fn node_color(&self) -> &str {
        "palegreen"
    }
}
#[cfg(test)]
mod test {
    use assert_matches::assert_matches;
    use std::path::Path;

    use super::*;
    use crate::aperture::Aperture;
    #[test]
    fn default() {
        let node = ParaxialSurface::default();
        assert_eq!(node.properties().name().unwrap(), "paraxial surface");
        assert_eq!(node.properties().node_type().unwrap(), "paraxial");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert!(node.properties().get("focal length").is_ok());
        assert_matches!(
            node.properties().get("focal length").unwrap(),
            Proptype::F64(_)
        );
        if let Ok(Proptype::F64(dist)) = node.properties().get("focal length") {
            assert_eq!(dist, &1.0);
        }
        assert_eq!(node.node_color(), "palegreen");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let node = ParaxialSurface::new("Test", Length::new::<millimeter>(1.0)).unwrap();
        assert_eq!(node.properties().name().unwrap(), "Test");
        if let Ok(Proptype::F64(dist)) = node.properties().get("focal length") {
            assert_eq!(dist, &1.0);
        }
        assert!(ParaxialSurface::new("Test", Length::new::<millimeter>(-1.0)).is_ok());
        assert!(ParaxialSurface::new("Test", Length::new::<millimeter>(0.0)).is_err());
        assert!(ParaxialSurface::new("Test", Length::new::<millimeter>(f64::NAN)).is_err());
        assert!(ParaxialSurface::new("Test", Length::new::<millimeter>(f64::INFINITY)).is_err());
        assert!(
            ParaxialSurface::new("Test", Length::new::<millimeter>(f64::NEG_INFINITY)).is_err()
        );
    }
    #[test]
    fn name_property() {
        let mut node = ParaxialSurface::default();
        node.set_property("name", "Test1".into()).unwrap();
        assert_eq!(node.properties().name().unwrap(), "Test1")
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
    fn set_input_aperture() {
        let mut node = ParaxialSurface::default();
        let aperture = Aperture::default();
        assert!(node.set_input_aperture("front", aperture.clone()).is_ok());
        assert!(node.set_input_aperture("rear", aperture.clone()).is_err());
        assert!(node.set_input_aperture("no port", aperture).is_err());
    }
    #[test]
    fn set_output_aperture() {
        let mut node = ParaxialSurface::default();
        let aperture = Aperture::default();
        assert!(node.set_output_aperture("rear", aperture.clone()).is_ok());
        assert!(node.set_output_aperture("front", aperture.clone()).is_err());
        assert!(node.set_output_aperture("no port", aperture).is_err());
    }
    #[test]
    fn export_data() {
        assert!(ParaxialSurface::default()
            .export_data(Path::new(""))
            .is_ok());
    }
    #[test]
    fn as_ref_node_mut() {
        let mut node = ParaxialSurface::default();
        assert!(node.as_refnode_mut().is_err());
    }
}
