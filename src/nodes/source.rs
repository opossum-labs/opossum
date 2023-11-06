#![warn(missing_docs)]
use std::collections::HashMap;
use std::fmt::Debug;

use crate::{
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
};

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
        .create("light data", "data of the emitted light", None, None.into())
        .unwrap();
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
    /// ## Example
    ///
    /// ```rust
    /// use opossum::{
    /// lightdata::{DataEnergy, LightData},
    /// nodes::Source,
    /// spectrum::create_he_ne_spectrum};
    ///
    /// let source=Source::new("My Source", LightData::Energy(DataEnergy {spectrum: create_he_ne_spectrum(1.0)}));
    /// ```
    pub fn new(name: &str, light: LightData) -> Self {
        let mut props = create_default_props();
        props.set("name", name.into()).unwrap();
        props
            .set_unchecked("light data", Some(light.clone()).into())
            .unwrap();
        Source { props }
    }

    /// Sets the light data of this [`Source`]. The [`LightData`] provided here represents the input data of an `OpticScenery`.
    pub fn set_light_data(&mut self, light_data: LightData) {
        self.props
            .set("light data", Some(light_data.clone()).into())
            .unwrap();
    }
}

impl Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let light_prop = self.props.get("light data").unwrap();
        let data = if let Proptype::LightData(data) = &light_prop {
            data
        } else {
            &None
        };
        match data {
            Some(data) => write!(f, "{}", data),
            None => write!(f, "no data"),
        }
    }
}

impl Optical for Source {
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_output("out1").unwrap();
        ports
    }
    fn analyze(
        &mut self,
        _incoming_edges: LightResult,
        _analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> OpmResult<LightResult> {
        let light_prop = self.props.get("light data").unwrap();
        let data = if let Proptype::LightData(data) = &light_prop {
            data
        } else {
            &None
        };
        if data.is_some() {
            Ok(HashMap::from([("out1".into(), data.to_owned())]))
        } else {
            Err(OpossumError::Analysis("no light data defined".into()))
        }
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        if name != "inverted" {
            self.props.set(name, prop)
        } else {
            let inverted = if let Proptype::Bool(inverted) = prop {
                inverted
            } else {
                false
            };
            if inverted {
                Err(OpossumError::Properties(
                    "Cannot change the inversion status of a source node!".into(),
                ))
            } else {
                Ok(())
            }
        }
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
    use crate::{analyzer::AnalyzerType, lightdata::DataEnergy, spectrum::create_he_ne_spectrum};
    #[test]
    fn default() {
        let node = Source::default();
        assert_eq!(node.properties().name().unwrap(), "source");
        assert_eq!(node.properties().node_type().unwrap(), "light source");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted(), false);
        assert_eq!(node.node_color(), "slateblue");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let source = Source::new("test", LightData::Fourier);
        assert_eq!(source.properties().name().unwrap(), "test");
    }
    #[test]
    fn not_invertable() {
        let mut node = Source::default();
        assert!(node.set_property("inverted", false.into()).is_ok());
        assert!(node.set_property("inverted", true.into()).is_err());
    }
    #[test]
    fn ports() {
        let node = Source::default();
        assert!(node.ports().input_names().is_empty());
        assert_eq!(node.ports().output_names(), vec!["out1"]);
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
            spectrum: create_he_ne_spectrum(1.0),
        });
        let mut node = Source::new("test", light.clone());
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
}
