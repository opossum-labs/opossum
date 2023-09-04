#![warn(missing_docs)]
use std::collections::HashMap;
use std::fmt::Debug;

use crate::{
    error::OpossumError,
    lightdata::LightData,
    optic_node::{Dottable, LightResult, Optical},
    optic_ports::OpticPorts,
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
#[derive(Default)]
pub struct Source {
    light_data: Option<LightData>,
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
    /// let source=Source::new(LightData::Energy(DataEnergy {spectrum: create_he_ne_spectrum(1.0)}));
    /// ```
    pub fn new(light: LightData) -> Self {
        Source {
            light_data: Some(light),
        }
    }

    /// Returns the light data of this [`Source`].
    pub fn light_data(&self) -> Option<&LightData> {
        self.light_data.as_ref()
    }

    /// Sets the light data of this [`Source`]. The [`LightData`] provided here represents the input data of an `OpticScenery`.
    pub fn set_light_data(&mut self, light_data: LightData) {
        self.light_data = Some(light_data);
    }
}

impl Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.light_data {
            Some(data) => write!(f, "{}", data),
            None => write!(f, "no data"),
        }
    }
}

impl Optical for Source {
    fn node_type(&self) -> &str {
        "light source"
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
        let data = self.light_data.clone();
        if data.is_some() {
            Ok(HashMap::from([("out1".into(), data)]))
        } else {
            Err(OpossumError::Analysis("no input data available".into()))
        }
    }
}

impl Dottable for Source {
    fn node_color(&self) -> &str {
        "slateblue"
    }
}
