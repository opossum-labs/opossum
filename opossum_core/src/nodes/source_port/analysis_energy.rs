use crate::{
    analyzers::energy::{AnalysisEnergy, EnergyConfig},
    core_optics::{NodeAttrExt, node_attr::HasNodeAttr},
    error::OpossumError,
    light::{LightData, LightResult},
    nodes::SourcePort,
};

impl AnalysisEnergy for SourcePort {
    fn analyze(
        &mut self,
        _incoming_data: LightResult,
        config: &EnergyConfig,
    ) -> crate::prelude::OpmResult<LightResult> {
        // If the source port is inverted it acts as sink and does not emit any rays (since then it has no outgoing ports).
        if self.inverted() {
            return Ok(LightResult::default());
        }
        let energy_data_builder = config.get_source(&self.node_attr().uuid()).ok_or_else(|| {
            OpossumError::Analysis(format!("No source data found in analyzer for {self}"))
        })?;
        Ok(LightResult::from([(
            "output_1".into(),
            LightData::Energy(energy_data_builder.build()?),
        )]))
    }
}

#[cfg(test)]
mod test {
    use crate::{
        error::OpmResult, light::spectrum_helper::create_he_ne_spec, prelude::EnergyDataBuilder,
    };

    use super::*;
    #[test]
    fn analyze_energy_no_source_definition() {
        let mut node = SourcePort::default();
        let output_error =
            AnalysisEnergy::analyze(&mut node, LightResult::default(), &EnergyConfig::default())
                .unwrap_err();
        assert!(
            output_error
                .to_string()
                .contains("No source data found in analyzer for")
        );
    }
    #[test]
    fn analyze_energy_ok() -> OpmResult<()> {
        let light_builder = EnergyDataBuilder::Raw(create_he_ne_spec(1.0)?.into());
        let mut node = SourcePort::default();
        let mut config = EnergyConfig::default();
        config.map_source(node.node_attr().uuid(), light_builder.clone());
        let output = AnalysisEnergy::analyze(&mut node, LightResult::default(), &config)?;
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let LightData::Energy(spectrum) = output.clone().unwrap() else {
            panic!("wrong type for output")
        };
        assert_eq!(*spectrum, light_builder.build()?);
        Ok(())
    }
}
