use crate::{
    analyzers::energy::AnalysisEnergy, error::OpmResult, light_result::LightResult,
    optic_node::OpticNode,
};

use super::Lens;

impl AnalysisEnergy for Lens {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let (inport, outport) = if self.inverted() {
            ("out1", "in1")
        } else {
            ("in1", "out1")
        };
        let Some(data) = incoming_data.get(inport) else {
            return Ok(LightResult::default());
        };
        Ok(LightResult::from([(outport.into(), data.clone())]))
    }
}
