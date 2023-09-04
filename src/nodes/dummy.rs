#![warn(missing_docs)]
use std::collections::HashMap;

use crate::analyzer::AnalyzerType;
use crate::error::OpossumError;
use crate::optic_node::{Dottable, LightResult, Optical};
use crate::optic_ports::OpticPorts;

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Debug, Default)]
/// A fake / dummy component without any optical functionality.
///
/// Any [`LightResult`] is directly forwarded without any modification. It is mainly used for
/// development and debugging purposes.
///
/// ## Optical Ports
///   - Inputs
///     - `front`
///   - Outputs
///     - `rear`
pub struct Dummy {
    is_inverted: bool,
}

impl Optical for Dummy {
    /// Returns "dummy" as node type.
    fn node_type(&self) -> &str {
        "dummy"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("front").unwrap();
        ports.add_output("rear").unwrap();
        ports
    }

    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _analyzer_type: &AnalyzerType,
    ) -> Result<LightResult> {
        if !self.is_inverted {
            if let Some(data) = incoming_data.get("front") {
                Ok(HashMap::from([("rear".into(), data.clone())]))
            } else {
                Ok(HashMap::from([("rear".into(), None)]))
            }
        } else if let Some(data) = incoming_data.get("rear") {
            Ok(HashMap::from([("front".into(), data.clone())]))
        } else {
            Ok(HashMap::from([("front".into(), None)]))
        }
    }
    fn set_inverted(&mut self, inverted: bool) {
        self.is_inverted = inverted;
    }
}

impl Dottable for Dummy {}
