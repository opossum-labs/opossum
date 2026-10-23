#![warn(missing_docs)]

use super::NodeGroup;
use crate::{
    analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
    core_optics::OpticNode,
    error::OpmResult,
    light::LightResult,
};

impl AnalysisRayTrace for NodeGroup {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        self.graph.analyze_raytrace(&incoming_data, config)
    }

    fn calc_node_positions(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        self.graph
            .set_external_distances(self.input_port_distances.clone());
        let result = self.graph.calc_node_positions(&incoming_data, config);
        self.reset_data();
        result
    }
}
