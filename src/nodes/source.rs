#![warn(missing_docs)]
use std::collections::HashMap;
use std::fmt::Debug;

use crate::{
    dottable::Dottable,
    error::OpossumError,
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Property, Proptype},
};

type Result<T> = std::result::Result<T, OpossumError>;

/// This node represents a source of light.
///
/// Hence it has only one output port (out1) and no input ports. Source nodes usually are the first nodes of an [`OpticScenery`](crate::OpticScenery).
///
/// ## Optical Ports
///   - Inputs
///     - none
///   - Outputs
///     - `out1`
pub struct Source {
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props.set(
        "name",
        Property {
            prop: Proptype::String("source".into()),
        },
    );
    props.set(
        "light data",
        Property {
            prop: Proptype::LightData(None),
        },
    );
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
        props.set(
            "name",
            Property {
                prop: Proptype::String(name.into()),
            },
        );
        props.set(
            "light data",
            Property {
                prop: Proptype::LightData(Some(light.clone())),
            },
        );
        Source { props }
    }

    /// Sets the light data of this [`Source`]. The [`LightData`] provided here represents the input data of an `OpticScenery`.
    pub fn set_light_data(&mut self, light_data: LightData) {
        self.props.set(
            "light data",
            Property {
                prop: Proptype::LightData(Some(light_data.clone())),
            },
        );
    }
}

impl Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let light_prop = self.props.get("light data").unwrap();
        let data = if let Proptype::LightData(data) = &light_prop.prop {
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
    fn node_type(&self) -> &str {
        "light source"
    }
    fn name(&self) -> &str {
        if let Proptype::String(name) = &self.props.get("name").unwrap().prop {
            name
        } else {
            "light source"
        }
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_output("out1").unwrap();
        ports
    }

    fn analyze(
        &mut self,
        _incoming_edges: LightResult,
        _analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> Result<LightResult> {
        let light_prop = self.props.get("light data").unwrap();
        let data = if let Proptype::LightData(data) = &light_prop.prop {
            data
        } else {
            &None
        };
        if data.is_some() {
            Ok(HashMap::from([("out1".into(), data.to_owned())]))
        } else {
            Err(OpossumError::Analysis("no input data available".into()))
        }
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Property) -> Result<()> {
        if self.props.set(name, prop).is_none() {
            Err(OpossumError::Other("property not defined".into()))
        } else {
            Ok(())
        }
    }
}

impl Dottable for Source {
    fn node_color(&self) -> &str {
        "slateblue"
    }
}
