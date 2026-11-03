use super::BeamSplitter;
use crate::{
    analyzers::{GhostFocusConfig, ghostfocus::AnalysisGhostFocus},
    error::OpmResult,
    light::{LightRays, Rays},
    nodes::splitter::Splitter,
};

impl AnalysisGhostFocus for BeamSplitter {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        self.split_ghost_bundles(incoming_data, config)
    }
}
