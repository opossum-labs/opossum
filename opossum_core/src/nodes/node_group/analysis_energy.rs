#![warn(missing_docs)]
use super::NodeGroup;
use crate::{
    analyzers::energy::{AnalysisEnergy, EnergyConfig},
    error::OpmResult,
    light::LightResult,
};

impl AnalysisEnergy for NodeGroup {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &EnergyConfig,
    ) -> OpmResult<LightResult> {
        self.graph.analyze_energy(&incoming_data, config)
    }
}
