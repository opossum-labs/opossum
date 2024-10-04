use crate::{
    analyzers::energy::AnalysisEnergy, error::OpmResult, light_result::LightResult,
    optic_node::OpticNode,
};

use super::Wedge;

impl AnalysisEnergy for Wedge {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let (in_port, out_port) = if self.inverted() {
            ("rear", "front")
        } else {
            ("front", "rear")
        };
        let Some(data) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        Ok(LightResult::from([(out_port.into(), data.clone())]))
    }
}
