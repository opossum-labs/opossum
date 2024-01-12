#![warn(missing_docs)]
//! Free-space propagation along the optical axis
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
};
use std::collections::HashMap;
use uom::si::{f64::Length, length::millimeter};
/// Propagation along the optical axis (z-Axis)
///
/// This node represents a free-space propagation along the optical axis (z-axis). So far,
/// the light propgates without a medium (vaccuum, refractive index = 1.0). The given `distance` corresponds to the
/// projected length on the optical axis.
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
///   - `distance`
#[derive(Debug, Clone)]
pub struct Propagation {
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut ports = OpticPorts::new();
    ports.create_input("front").unwrap();
    ports.create_output("rear").unwrap();
    let mut props = Properties::new("propagation", "propagation");
    props
        .create(
            "distance",
            "distance along the optical axis in mm",
            None,
            0.0.into(),
        )
        .unwrap();
    props.set("apertures", ports.into()).unwrap();
    props
}
impl Default for Propagation {
    fn default() -> Self {
        Self {
            props: create_default_props(),
        }
    }
}
impl Propagation {
    /// Create a new propagation node of the given length.
    ///
    ///
    /// # Errors
    /// This function returns an error if
    ///  - the given `length_along_z` is not finite.
    /// # Panics
    /// This function panics if
    /// - the input port name already exists. (Theoretically impossible at this point, as the [`OpticPorts`] are created just before in this function)
    /// - the output port name already exists. (Theoretically impossible at this point, as the [`OpticPorts`] are created just before in this function)
    pub fn new(name: &str, length_along_z: Length) -> OpmResult<Self> {
        if !length_along_z.is_normal() {
            return Err(OpossumError::Other(
                "propagation length must be finite".into(),
            ));
        }
        let mut props = create_default_props();
        props.set("name", name.into())?;
        props.set("distance", length_along_z.get::<millimeter>().into())?;
        Ok(Self { props })
    }
}
impl Optical for Propagation {
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
                    let length_along_z =
                        if let Ok(Proptype::F64(length)) = self.props.get("distance") {
                            *length
                        } else {
                            return Err(OpossumError::Analysis("cannot read distance".into()));
                        };
                    rays.propagate_along_z(Length::new::<millimeter>(length_along_z))?;
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

impl Dottable for Propagation {
    fn node_color(&self) -> &str {
        "none"
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
        let node = Propagation::default();
        assert_eq!(node.properties().name().unwrap(), "propagation");
        assert_eq!(node.properties().node_type().unwrap(), "propagation");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert!(node.properties().get("distance").is_ok());
        assert_matches!(node.properties().get("distance").unwrap(), Proptype::F64(_));
        if let Ok(Proptype::F64(dist)) = node.properties().get("distance") {
            assert_eq!(dist, &0.0);
        }
        assert_eq!(node.node_color(), "none");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        assert!(Propagation::new("Test", Length::new::<millimeter>(f64::INFINITY)).is_err());
        let node = Propagation::new("Test", Length::new::<millimeter>(1.0)).unwrap();
        assert_eq!(node.properties().name().unwrap(), "Test");
        if let Ok(Proptype::F64(dist)) = node.properties().get("distance") {
            assert_eq!(dist, &1.0);
        }
    }
    #[test]
    fn name_property() {
        let mut node = Propagation::default();
        node.set_property("name", "Test1".into()).unwrap();
        assert_eq!(node.properties().name().unwrap(), "Test1")
    }
    #[test]
    fn node_type_readonly() {
        let mut node = Propagation::default();
        assert!(node.set_property("node_type", "other".into()).is_err());
    }
    #[test]
    fn inverted() {
        let mut node = Propagation::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.properties().inverted().unwrap(), true)
    }
    #[test]
    fn ports() {
        let node = Propagation::default();
        assert_eq!(node.ports().input_names(), vec!["front"]);
        assert_eq!(node.ports().output_names(), vec!["rear"]);
    }
    #[test]
    fn set_input_aperture() {
        let mut node = Propagation::default();
        let aperture = Aperture::default();
        assert!(node.set_input_aperture("front", aperture.clone()).is_ok());
        assert!(node.set_input_aperture("rear", aperture.clone()).is_err());
        assert!(node.set_input_aperture("no port", aperture).is_err());
    }
    #[test]
    fn set_output_aperture() {
        let mut node = Propagation::default();
        let aperture = Aperture::default();
        assert!(node.set_output_aperture("rear", aperture.clone()).is_ok());
        assert!(node.set_output_aperture("front", aperture.clone()).is_err());
        assert!(node.set_output_aperture("no port", aperture).is_err());
    }
    // #[test]
    // #[ignore]
    // fn export_data() {
    //     assert!(Propagation::default().export_data(Path::new("")).is_ok());
    // }
    #[test]
    fn as_ref_node_mut() {
        let mut node = Propagation::default();
        assert!(node.as_refnode_mut().is_err());
    }
}
