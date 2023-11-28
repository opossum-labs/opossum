use std::collections::HashMap;

use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::OpmResult,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
};

#[derive(Debug, Clone)]
pub struct Propagation {
    props: Properties,
}

impl Default for Propagation {
    fn default() -> Self {
        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        let mut props = Properties::new("propagation", "propagation");
        props.set("apertures", ports.into()).unwrap();
        Self { props }
    }
}

impl Optical for Propagation {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (src, target) = if self.properties().inverted()? {
            ("rear", "front")
        } else {
            ("front", "rear")
        };
        let data = incoming_data.get(src).unwrap_or(&None);
        Ok(HashMap::from([(target.into(), data.clone())]))
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.props.set(name, prop)
    }
}

impl Dottable for Propagation {}
