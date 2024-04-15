#![warn(missing_docs)]
//! Free-space propagation along the optical axis
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::Proptype,
    refractive_index::{refr_index_vaccuum, RefractiveIndexType},
    utils::EnumProxy,
};
use num::Zero;
use uom::si::f64::Length;

use super::node_attr::NodeAttr;
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
///   - `refractive index`
#[derive(Debug, Clone)]
pub struct Propagation {
    node_attr: NodeAttr,
}
impl Default for Propagation {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("propagation", "propagation");
        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        node_attr.set_property("apertures", ports.into()).unwrap();
        node_attr
            .create_property(
                "distance",
                "distance along the optical axis in mm",
                None,
                Length::zero().into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "refractive index",
                "refractive index of the medium",
                None,
                EnumProxy::<RefractiveIndexType> {
                    value: refr_index_vaccuum(),
                }
                .into(),
            )
            .unwrap();
        Self { node_attr }
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
        let mut propa = Self::default();
        propa.node_attr.set_property("name", name.into())?;
        propa
            .node_attr
            .set_property("distance", length_along_z.into())?;
        Ok(propa)
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
        let Some(data) = incoming_data.get(src) else {
            return Ok(LightResult::default());
        };
        let light_data = match analyzer_type {
            AnalyzerType::Energy => data.clone(),
            AnalyzerType::RayTrace(_config) => {
                if let LightData::Geometric(mut rays) = data.clone() {
                    let Ok(Proptype::Length(length_along_z)) =
                        self.node_attr.get_property("distance")
                    else {
                        return Err(OpossumError::Analysis("cannot read distance".into()));
                    };
                    let Ok(Proptype::RefractiveIndex(refractive_index)) =
                        self.node_attr.get_property("refractive index")
                    else {
                        return Err(OpossumError::Analysis(
                            "cannot read refractive index".into(),
                        ));
                    };
                    rays.set_refractive_index(&refractive_index.value)?;
                    let previous_dist_to_next_surface = rays.dist_to_next_surface();
                    rays.set_dist_to_next_surface(previous_dist_to_next_surface + *length_along_z);
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

impl Dottable for Propagation {
    fn node_color(&self) -> &str {
        "none"
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{aperture::Aperture, millimeter, nodes::test_helper::test_helper::*, rays::Rays};
    use assert_matches::assert_matches;
    #[test]
    fn default() {
        let node = Propagation::default();
        assert_eq!(node.name(), "propagation");
        assert_eq!(node.node_type(), "propagation");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert!(node.properties().get("distance").is_ok());
        assert_matches!(
            node.properties().get("distance").unwrap(),
            Proptype::Length(_)
        );
        if let Ok(Proptype::Length(dist)) = node.properties().get("distance") {
            assert_eq!(*dist, Length::zero());
        }
        assert_eq!(node.node_color(), "none");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        assert!(Propagation::new("Test", millimeter!(f64::INFINITY)).is_err());
        let node = Propagation::new("Test", millimeter!(1.0)).unwrap();
        assert_eq!(node.name(), "Test");
        if let Ok(Proptype::F64(dist)) = node.properties().get("distance") {
            assert_eq!(dist, &1.0);
        }
    }
    #[test]
    fn name_property() {
        let mut node = Propagation::default();
        node.set_property("name", "Test1".into()).unwrap();
        assert_eq!(node.name(), "Test1")
    }
    #[test]
    fn node_type_readonly() {
        let mut node = Propagation::default();
        assert!(node.set_property("node_type", "other".into()).is_err());
    }
    #[test]
    fn inverted() {
        test_inverted::<Propagation>()
    }
    #[test]
    fn ports() {
        let node = Propagation::default();
        assert_eq!(node.ports().input_names(), vec!["front"]);
        assert_eq!(node.ports().output_names(), vec!["rear"]);
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<Propagation>()
    }
    #[test]
    fn analyze_wrong_port() {
        let mut node = Propagation::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("rear".into(), input_light.clone());
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn set_input_aperture() {
        let mut node = Propagation::default();
        let aperture = Aperture::default();
        assert!(node.set_input_aperture("front", &aperture).is_ok());
        assert!(node.set_input_aperture("rear", &aperture).is_err());
        assert!(node.set_input_aperture("no port", &aperture).is_err());
    }
    #[test]
    fn set_output_aperture() {
        let mut node = Propagation::default();
        let aperture = Aperture::default();
        assert!(node.set_output_aperture("rear", &aperture).is_ok());
        assert!(node.set_output_aperture("front", &aperture).is_err());
        assert!(node.set_output_aperture("no port", &aperture).is_err());
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
