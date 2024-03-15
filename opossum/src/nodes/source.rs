#![warn(missing_docs)]
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
    utils::EnumProxy,
};
use std::collections::HashMap;
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
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut props = Properties::new("source", "light source");
    props
        .create(
            "light data",
            "data of the emitted light",
            None,
            EnumProxy::<Option<LightData>> { value: None }.into(),
        )
        .unwrap();
    let mut ports = OpticPorts::new();
    ports.create_output("out1").unwrap();
    props.set("apertures", ports.into()).unwrap();
    props
}

impl Default for Source {
    fn default() -> Self {
        Self {
            props: create_default_props(),
        }
    }
}
impl Source {
    /// Creates a new [`Source`].
    ///
    /// The light to be emitted from this source is defined in a [`LightData`] structure.
    ///
    /// # Panics
    /// Panics if [`Properties`] `name` can not be set
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
        let mut props = create_default_props();
        props.set("name", name.into()).unwrap();
        props
            .set_unchecked(
                "light data",
                EnumProxy::<Option<LightData>> {
                    value: Some(light.clone()),
                }
                .into(),
            )
            .unwrap();
        Self { props }
    }

    /// Sets the light data of this [`Source`]. The [`LightData`] provided here represents the input data of an `OpticScenery`.
    ///
    /// # Attributes
    /// * `light_data`: [`LightData`] that shall be set
    ///
    /// # Errors
    /// This function returns an error if the property "light data" can not be set
    pub fn set_light_data(&mut self, light_data: &LightData) -> OpmResult<()> {
        self.props.set(
            "light data",
            EnumProxy::<Option<LightData>> {
                value: Some(light_data.clone()),
            }
            .into(),
        )?;
        Ok(())
    }
}

impl Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let light_prop = self.props.get("light data").unwrap();
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
        if let Ok(Proptype::LightData(data)) = self.props.get("light data") {
            let Some(mut data) = data.value.clone() else {
                return Err(OpossumError::Analysis(
                    "source has empty light data defined".into(),
                ));
            };
            if let LightData::Geometric(rays) = &mut data {
                if let Some(aperture) = self.ports().output_aperture("out1") {
                    rays.apodize(aperture)?;
                    if let AnalyzerType::RayTrace(config) = analyzer_type {
                        rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                    }
                } else {
                    return Err(OpossumError::OpticPort("input aperture not found".into()));
                };
            }
            Ok(HashMap::from([("out1".into(), Some(data))]))
        } else {
            Err(OpossumError::Analysis(
                "source has no light data defined".into(),
            ))
        }
    }
    fn properties(&self) -> &Properties {
        &self.props
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
                } else {
                    return Ok(());
                }
            };
        };
        self.props.set(name, prop)
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
        assert_eq!(node.properties().name().unwrap(), "source");
        assert_eq!(node.properties().node_type().unwrap(), "light source");
        if let Ok(Proptype::LightData(light_data)) = node.properties().get("light data") {
            assert_eq!(light_data.value, None);
        } else {
            panic!("cannot unpack light data property");
        };
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "slateblue");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let source = Source::new("test", &LightData::Fourier);
        assert_eq!(source.properties().name().unwrap(), "test");
    }
    #[test]
    fn not_invertable() {
        let mut node = Source::default();
        assert!(node.set_property("inverted", false.into()).is_ok());
        assert!(node.set_property("inverted", true.into()).is_err());
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
    fn analyze_empty() {
        let mut node = Source::default();
        let incoming_data: LightResult = LightResult::default();
        assert!(node.analyze(incoming_data, &AnalyzerType::Energy).is_err())
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
        let output = output.get("out1").unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, light);
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
