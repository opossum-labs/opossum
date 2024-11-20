//! Performing a (simple) energy flow analysis

use super::Analyzer;
use crate::{
    error::OpmResult, light_result::LightResult, nodes::NodeGroup, optic_node::OpticNode,
    reporting::analysis_report::AnalysisReport,
};
use log::info;

/// Analyzer for simulating a simple energy flow
#[derive(Debug, Default)]
pub struct EnergyAnalyzer {}

impl Analyzer for EnergyAnalyzer {
    fn analyze(&self, scenery: &mut NodeGroup) -> OpmResult<()> {
        let scenery_name = if scenery.node_attr().name().is_empty() {
            String::new()
        } else {
            format!(" '{}'", scenery.node_attr().name())
        };
        info!("Performing energy flow analysis of scenery{scenery_name}.");
        AnalysisEnergy::analyze(scenery, LightResult::default())?;
        Ok(())
    }
    fn report(&self, scenery: &NodeGroup) -> OpmResult<AnalysisReport> {
        let mut report = scenery.toplevel_report()?;
        report.set_analysis_type("Energy Analysis");
        Ok(report)
    }
}
/// Trait for implementing the energy flow analysis.
pub trait AnalysisEnergy: OpticNode {
    /// Analyze the energy flow of an [`OpticNode`].
    ///
    /// # Errors
    /// This function will return an error if the concrete implementation of the [`OpticNode`] fails.
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult>;
}

#[cfg(test)]
mod test {
    use num::Zero;
    use uom::si::f64::Length;

    use super::EnergyAnalyzer;
    use crate::{
        analyzers::Analyzer,
        lightdata::{DataEnergy, LightData},
        nodes::{EnergyMeter, NodeGroup, Source},
        spectrum_helper::create_he_ne_spec,
    };

    #[test]
    fn analyze_empty_scene() {
        let mut scenery = NodeGroup::default();
        let energy_analyzer = EnergyAnalyzer {};
        energy_analyzer.analyze(&mut scenery).unwrap();
    }
    fn create_scene() -> NodeGroup {
        let mut scenery = NodeGroup::default();
        let data_energy = DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        };
        let light_data = LightData::Energy(data_energy);
        let src = Source::new("source", &light_data);
        let i_src = scenery.add_node(&src).unwrap();
        let i_em = scenery.add_node(&EnergyMeter::default()).unwrap();
        scenery
            .connect_nodes(i_src, "output_1", i_em, "input_1", Length::zero())
            .unwrap();
        scenery
    }
    #[test]
    fn analyze_full_scene() {
        let mut scenery = create_scene();
        let energy_analyzer = EnergyAnalyzer {};
        energy_analyzer.analyze(&mut scenery).unwrap();
    }
    #[test]
    fn report_without_analysis() {
        let mut scenery = create_scene();
        let energy_analyzer = EnergyAnalyzer {};
        energy_analyzer.analyze(&mut scenery).unwrap();
        energy_analyzer.report(&mut scenery).unwrap();
    }
}
