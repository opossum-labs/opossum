#![warn(missing_docs)]
use crate::lightdata::LightData;
use crate::properties::{Properties, Property};
use crate::{
    dottable::Dottable,
    error::OpossumError,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
};
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, OpossumError>;

/// This node represents an universal detector (so far for test / debugging purposes).
///
/// Any [`LightData`] coming in will be stored internally for later display / export.
///
/// ## Optical Ports
///   - Inputs
///     - `in1`
///   - Outputs
///     - `out1`
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere in between arbitrary optic nodes.
pub struct Detector {
    light_data: Option<LightData>,
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props.set("name", "detector".into());
    props
}
impl Default for Detector {
    fn default() -> Self {
        Self {
            light_data: Default::default(),
            props: create_default_props(),
        }
    }
}
impl Optical for Detector {
    fn node_type(&self) -> &str {
        "detector"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("in1").unwrap();
        ports.add_output("out1").unwrap();
        ports
    }
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> Result<LightResult> {
        if let Some(data) = incoming_data.get("in1") {
            self.light_data = data.clone();
            Ok(HashMap::from([("out1".into(), data.clone())]))
        } else {
            Ok(HashMap::from([("out2".into(), None)]))
        }
    }
    fn export_data(&self, report_dir: &Path) {
        if let Some(data) = &self.light_data {
            let mut file_path = PathBuf::from(report_dir);
            file_path.push(format!("spectrum_{}.svg", self.name()));
            data.export(&file_path)
        }
    }
    fn is_detector(&self) -> bool {
        true
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

impl Debug for Detector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.light_data {
            Some(data) => write!(f, "{}", data),
            None => write!(f, "no data"),
        }
    }
}
impl Dottable for Detector {
    fn node_color(&self) -> &str {
        "lemonchiffon"
    }
}
