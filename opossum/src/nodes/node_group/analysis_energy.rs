#![warn(missing_docs)]
use super::NodeGroup;
use crate::{analyzers::energy::AnalysisEnergy, error::OpmResult, light_result::LightResult};

impl AnalysisEnergy for NodeGroup {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        self.graph.analyze_energy(&incoming_data)
    }
}
