use crate::{
    analyzers::energy::{AnalysisEnergy, EnergyConfig},
    error::OpossumError,
    light_result::LightResult,
    lightdata::LightData,
    nodes::SourcePort,
    prelude::OpticNode,
};

impl AnalysisEnergy for SourcePort {
    fn analyze(
        &mut self,
        _incoming_data: LightResult,
        config: &EnergyConfig,
    ) -> crate::prelude::OpmResult<LightResult> {
        let energy_data_builder = config.get_source(&self.node_attr().uuid()).ok_or_else(|| {
            OpossumError::Analysis(format!("No source data found in analyzer for {self}"))
        })?;
        Ok(LightResult::from([(
            "output_1".into(),
            LightData::Energy(energy_data_builder.build()?),
        )]))
    }
}
