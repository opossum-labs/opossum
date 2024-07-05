#![warn(missing_docs)]
use super::node_attr::NodeAttr;
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{Alignable, LightResult, Optical},
    properties::Proptype,
    utils::{geom_transformation::Isometry, EnumProxy},
};
use std::fmt::Debug;

/// A general light source
///
/// Hence it has only one output port (out1) and no input ports. Source nodes usually are the first nodes of an [`OpticScenery`](crate::OpticScenery).
///
/// ## Optical Ports
///   - Inputs
///     - none
///   - Outputs
///     - `out1`
///
/// ## Properties
///   - `name`
///   - `light data`
///
/// **Note**: This node does not have the `inverted` property since it has only one output port.
pub struct Source {
    node_attr: NodeAttr,
}
impl Default for Source {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("source");
        node_attr
            .create_property(
                "light data",
                "data of the emitted light",
                None,
                EnumProxy::<Option<LightData>> { value: None }.into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "isometry",
                "absolute node location / orientation",
                None,
                EnumProxy::<Option<Isometry>> { value: None }.into(),
            )
            .unwrap();
        let mut ports = OpticPorts::new();
        ports.create_output("out1").unwrap();
        node_attr.set_property("apertures", ports.into()).unwrap();
        Self { node_attr }
    }
}
impl Source {
    /// Creates a new [`Source`].
    ///
    /// The light to be emitted from this source is defined in a [`LightData`] structure.
    ///
    /// # Panics
    /// Panics if [`Properties`](crate::properties::Properties) `name` can not be set
    ///
    /// ## Example
    ///
    /// ```rust
    /// use opossum::{
    /// lightdata::{DataEnergy, LightData},
    /// nodes::Source,
    /// spectrum_helper::create_he_ne_spec};
    ///
    /// let source=Source::new("My Source", &LightData::Energy(DataEnergy {spectrum: create_he_ne_spec(1.0).unwrap()}));
    /// ```
    #[must_use]
    pub fn new(name: &str, light: &LightData) -> Self {
        let mut source = Self::default();
        source.node_attr.set_property("name", name.into()).unwrap();
        source
            .node_attr
            .set_property(
                "light data",
                EnumProxy::<Option<LightData>> {
                    value: Some(light.clone()),
                }
                .into(),
            )
            .unwrap();
        source
    }

    /// Sets the light data of this [`Source`]. The [`LightData`] provided here represents the input data of an `OpticScenery`.
    ///
    /// # Attributes
    /// * `light_data`: [`LightData`] that shall be set
    ///
    /// # Errors
    /// This function returns an error if the property "light data" can not be set
    pub fn set_light_data(&mut self, light_data: &LightData) -> OpmResult<()> {
        self.node_attr.set_property(
            "light data",
            EnumProxy::<Option<LightData>> {
                value: Some(light_data.clone()),
            }
            .into(),
        )?;
        Ok(())
    }
}

impl Alignable for Source {}

impl Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let light_prop = self.node_attr.get_property("light data").unwrap();
        let data = if let Proptype::LightData(data) = &light_prop {
            &data.value
        } else {
            &None
        };
        match data {
            Some(data) => write!(f, "Source: {data}"),
            None => write!(f, "Source: no data"),
        }
    }
}
impl Optical for Source {
    fn analyze(
        &mut self,
        _incoming_edges: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        if let Ok(Proptype::LightData(data)) = self.node_attr.get_property("light data") {
            let Some(mut data) = data.value.clone() else {
                return Err(OpossumError::Analysis(
                    "source has empty light data defined".into(),
                ));
            };
            if let LightData::Geometric(rays) = &mut data {
                if let Some(iso) = self.effective_iso() {
                    *rays = rays.transformed_rays(&iso);
                }
                if let Some(aperture) = self.ports().output_aperture("out1") {
                    rays.apodize(aperture)?;
                    if let AnalyzerType::RayTrace(config) = analyzer_type {
                        rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                    }
                } else {
                    return Err(OpossumError::OpticPort("input aperture not found".into()));
                };
            }
            Ok(LightResult::from([("out1".into(), data)]))
        } else {
            Err(OpossumError::Analysis(
                "source has no light data defined".into(),
            ))
        }
    }
    fn is_source(&self) -> bool {
        true
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        if name == "inverted" {
            if let Proptype::Bool(inverted) = prop {
                if inverted {
                    return Err(OpossumError::Properties(
                        "Cannot change the inversion status of a source node!".into(),
                    ));
                }
                return Ok(());
            };
        };
        self.node_attr.set_property(name, prop)
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn after_deserialization_hook(&mut self) -> OpmResult<()> {
        // Synchronize iosmetry property.
        if let Proptype::Isometry(prox_iso) = &self.node_attr.get_property("isometry")? {
            if let Some(iso) = &prox_iso.value {
                self.set_isometry(iso.clone());
            }
        }
        Ok(())
    }
    fn set_isometry(&mut self, isometry: crate::utils::geom_transformation::Isometry) {
        self.node_attr.set_isometry(isometry.clone());
        self.set_property("isometry", Some(isometry).into())
            .unwrap();
    }
}

impl Dottable for Source {
    fn node_color(&self) -> &str {
        "slateblue"
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzer::AnalyzerType, lightdata::DataEnergy, spectrum_helper::create_he_ne_spec,
    };
    use assert_matches::assert_matches;

    #[test]
    fn default() {
        let node = Source::default();
        assert_eq!(node.name(), "source");
        assert_eq!(node.node_type(), "source");
        if let Ok(Proptype::LightData(light_data)) = node.properties().get("light data") {
            assert_eq!(light_data.value, None);
        } else {
            panic!("cannot unpack light data property");
        };
        assert_eq!(node.is_detector(), false);
        assert!(Source::default().is_source());
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "slateblue");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let source = Source::new("test", &LightData::Fourier);
        assert_eq!(source.name(), "test");
    }
    #[test]
    fn not_invertable() {
        let mut node = Source::default();
        assert!(node.set_inverted(false).is_ok());
        assert!(node.set_inverted(true).is_err());
        assert!(node.set_property("name", "blah".into()).is_ok());
    }
    #[test]
    fn ports() {
        let node = Source::default();
        assert!(node.ports().input_names().is_empty());
        assert_eq!(node.ports().output_names(), vec!["out1"]);
    }
    #[test]
    fn test_set_light_data() {
        let mut src = Source::default();
        if let Ok(Proptype::LightData(light_data)) = src.properties().get("light data") {
            assert_eq!(light_data.value, None);
        }
        src.set_light_data(&LightData::Fourier).unwrap();
        if let Ok(Proptype::LightData(light_data)) = src.properties().get("light data") {
            assert_matches!(light_data.value.clone().unwrap(), LightData::Fourier);
        }
    }
    #[test]
    fn analyze_no_light_defined() {
        let mut node = Source::default();
        let output = node.analyze(LightResult::default(), &AnalyzerType::Energy);
        assert!(output.is_err());
    }
    #[test]
    fn analyze_ok() {
        let light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        let mut node = Source::new("test", &light);
        let incoming_data: LightResult = LightResult::default();
        let output = node.analyze(incoming_data, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, light);
    }
    #[test]
    fn debug() {
        assert_eq!(format!("{:?}", Source::default()), "Source: no data");
        assert_eq!(
            format!("{:?}", Source::new("hallo", &LightData::Fourier)),
            "Source: No display defined for this type of LightData"
        );
    }
}
