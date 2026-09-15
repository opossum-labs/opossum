//! Execution and orchestration of optical analyses for [`OpmDocument`].

use super::OpmDocument;
use crate::{
    analyzers::{Analyzer, AnalyzerRegistration, AnalyzerType},
    core_optics::OpticNode,
    error::{OpmResult, OpossumError},
    reporting::analysis_report::AnalysisReport,
};
use log::info;

/// Internal description of a single planned analysis run.
#[derive(Debug, Clone)]
struct AnalysisRun {
    analyzer_nr: usize,
    analyzer_type: AnalyzerType,
    scenario_name: Option<String>,
    analyzer_name: Option<String>,
}

impl OpmDocument {
    /// Performs an analysis run of this [`OpmDocument`].
    ///
    /// Executes the defined analyzers in the order they were registered.
    /// An analyzer referring to [`PumpScenario`](crate::gain::PumpScenario)s runs once per scenario;
    /// an analyzer with no scenario specified runs once on the passive model.
    ///
    /// # Errors
    ///
    /// Returns an error if an analyzer refers to a non-existent pump scenario or if
    /// an individual analyzer execution fails.
    pub fn analyze(&mut self) -> OpmResult<Vec<AnalysisReport>> {
        if self.analyzers.is_empty() {
            info!("No analyzer defined in document. Stopping here.");
            return Ok(vec![]);
        }

        let runs = self.analysis_runs()?;
        let mut reports = Vec::new();

        for AnalysisRun {
            analyzer_nr,
            analyzer_type,
            scenario_name,
            analyzer_name,
        } in runs
        {
            let analyzer_box = inventory::iter::<AnalyzerRegistration>
                .into_iter()
                .find_map(|reg| (reg.builder)(&analyzer_type))
                .ok_or_else(|| {
                    OpossumError::Other(format!(
                        "No analyzer implementation found for type: {analyzer_type:?}"
                    ))
                })?;
            let analyzer: &dyn Analyzer = &*analyzer_box;

            match (&analyzer_name, &scenario_name) {
                (Some(name), Some(scenario)) => {
                    info!("Analysis #{analyzer_nr} '{name}', pump scenario '{scenario}'");
                }
                (Some(name), None) => {
                    info!("Analysis #{analyzer_nr} '{name}'");
                }
                (None, Some(scenario)) => {
                    info!("Analysis #{analyzer_nr}, pump scenario '{scenario}'");
                }
                (None, None) => {
                    info!("Analysis #{analyzer_nr}");
                }
            }

            analyzer.analyze(&mut self.scenery)?;
            info!("Generating report #{analyzer_nr}");
            let mut report = analyzer.report(&self.scenery)?;

            if let Some(name) = &scenario_name {
                report.set_analysis_type(&format!("{} - {name}", report.analysis_type()));
            }
            if let Some(name) = analyzer_name {
                report.set_analyzer_name(Some(name));
            }
            reports.push(report);

            self.scenery.clear_edges();
            self.scenery.reset_data();
        }

        Ok(reports)
    }

    /// Expands the analyzers of this [`OpmDocument`] into individual executable runs.
    ///
    /// Validates all scenario references upfront so that non-existent scenarios
    /// fail immediately before any optical calculations begin.
    ///
    /// # Errors
    ///
    /// Returns an error if an analyzer refers to a scenario ID not present in the document.
    fn analysis_runs(&self) -> OpmResult<Vec<AnalysisRun>> {
        let mut runs = Vec::new();

        for (analyzer_nr, (_, analyzer_info)) in self.analyzers.iter().enumerate() {
            let analyzer_name = analyzer_info.name().map(ToString::to_string);

            if analyzer_info.pump_scenarios().is_empty() {
                runs.push(AnalysisRun {
                    analyzer_nr,
                    analyzer_type: analyzer_info.analyzer_type().clone(),
                    scenario_name: None,
                    analyzer_name,
                });
                continue;
            }

            for scenario_id in analyzer_info.pump_scenarios() {
                let scenario = self.pump_scenarios.get(scenario_id).ok_or_else(|| {
                    OpossumError::OpmDocument(format!(
                        "analysis #{analyzer_nr} refers to the pump scenario {scenario_id}, \
                         which does not exist"
                    ))
                })?;

                let mut analyzer_type = analyzer_info.analyzer_type().clone();
                analyzer_type.set_active_pump_scenario(Some(scenario.clone()));

                runs.push(AnalysisRun {
                    analyzer_nr,
                    analyzer_type,
                    scenario_name: Some(scenario.name().to_string()),
                    analyzer_name: analyzer_name.clone(),
                });
            }
        }

        Ok(runs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analyzers::{AnalyzerType, energy::EnergyConfig},
        gain::{ConstGain, GainModel},
        joule,
        light::lightdata::energy_data_builder::{EnergyDataBuilder, EnergyLaserLines},
        millimeter, nanometer,
        nodes::{EnergyMeter, Lens, NodeGroup, SourcePort},
        utils::test_helper::test_helper::metered_energy,
    };
    use approx::assert_relative_eq;
    use uuid::Uuid;

    /// Helper to assemble a minimal document with one source, one meter, and an energy analyzer.
    fn document_with_one_analyzer() -> OpmResult<(OpmDocument, Uuid)> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let meter = scenery.add_node(EnergyMeter::default())?;
        scenery.connect_nodes(source, "output_1", meter, "input_1", millimeter!(10.0))?;

        let mut document = OpmDocument::new(scenery);
        let mut config = EnergyConfig::default();
        config.map_source(
            source,
            EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
                vec![(nanometer!(1053.0), joule!(1.0))],
                nanometer!(1.0),
            )?),
        );

        let analyzer_id = document.add_analyzer(AnalyzerType::Energy(config));
        Ok((document, analyzer_id))
    }

    /// Helper to extract all analysis type labels from generated reports.
    fn analysis_types_of(document: &mut OpmDocument) -> OpmResult<Vec<String>> {
        Ok(document
            .analyze()?
            .iter()
            .map(|report| report.analysis_type().to_string())
            .collect())
    }

    #[test]
    fn an_analyzer_without_scenarios_is_run_once() -> OpmResult<()> {
        let (mut document, _) = document_with_one_analyzer()?;
        assert_eq!(analysis_types_of(&mut document)?, vec!["Energy Analysis"]);
        Ok(())
    }

    #[test]
    fn an_analyzer_is_run_once_per_scenario() -> OpmResult<()> {
        let (mut document, analyzer_id) = document_with_one_analyzer()?;
        let full_power = document.add_pump_scenario("full power");
        let half_power = document.add_pump_scenario("half power");

        document
            .analyzer_mut(analyzer_id)
            .expect("analyzer just added must exist")
            .set_pump_scenarios(vec![full_power, half_power]);

        assert_eq!(
            analysis_types_of(&mut document)?,
            vec![
                "Energy Analysis - full power",
                "Energy Analysis - half power"
            ]
        );
        Ok(())
    }

    #[test]
    fn an_analyzer_pointing_at_a_missing_scenario_is_an_error() -> OpmResult<()> {
        let (mut document, analyzer_id) = document_with_one_analyzer()?;
        let missing_id = Uuid::new_v4();

        document
            .analyzer_mut(analyzer_id)
            .expect("analyzer just added must exist")
            .set_pump_scenarios(vec![missing_id]);

        let message = document.analyze().unwrap_err().to_string();
        assert!(
            message.contains(&missing_id.to_string()),
            "the error has to name the missing scenario, got: {message}"
        );
        Ok(())
    }

    #[test]
    fn two_scenarios_give_two_different_results() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let lens = scenery.add_node(Lens::default())?;
        let meter = scenery.add_node(EnergyMeter::default())?;

        scenery.connect_nodes(source, "output_1", lens, "input_1", millimeter!(10.0))?;
        scenery.connect_nodes(lens, "output_1", meter, "input_1", millimeter!(10.0))?;

        let mut document = OpmDocument::new(scenery);
        let mut config = EnergyConfig::default();
        config.map_source(
            source,
            EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
                vec![(nanometer!(1053.0), joule!(1.0))],
                nanometer!(1.0),
            )?),
        );
        let analyzer_id = document.add_analyzer(AnalyzerType::Energy(config));

        let mut scenario_ids = Vec::new();
        for (name, gain) in [("full power", 4.0), ("half power", 2.0)] {
            let scenario_id = document.add_pump_scenario(name);
            document
                .pump_scenario_mut(scenario_id)
                .expect("scenario just added must exist")
                .set_gain_model(lens, GainModel::Const(ConstGain::new(gain)?));
            scenario_ids.push(scenario_id);
        }

        document
            .analyzer_mut(analyzer_id)
            .expect("analyzer just added must exist")
            .set_pump_scenarios(scenario_ids);

        let reports = document.analyze()?;
        assert_eq!(reports.len(), 2);

        let full_power = metered_energy(&reports[0])?;
        let half_power = metered_energy(&reports[1])?;
        assert_relative_eq!(full_power / half_power, 2.0, epsilon = 1e-12);
        Ok(())
    }

    #[test]
    fn removing_a_scenario_stops_the_analyzers_running_it() -> OpmResult<()> {
        let (mut document, analyzer_id) = document_with_one_analyzer()?;
        let full_power = document.add_pump_scenario("full power");
        let half_power = document.add_pump_scenario("half power");

        document
            .analyzer_mut(analyzer_id)
            .expect("analyzer just added must exist")
            .set_pump_scenarios(vec![full_power, half_power]);

        assert!(document.remove_pump_scenario(full_power).is_some());
        assert_eq!(
            document.analyzer(analyzer_id)?.pump_scenarios(),
            vec![half_power]
        );
        assert_eq!(
            analysis_types_of(&mut document)?,
            vec!["Energy Analysis - half power"]
        );
        Ok(())
    }

    #[test]
    fn analyzer_name_propagates_to_analysis_report() -> OpmResult<()> {
        let (mut document, analyzer_id) = document_with_one_analyzer()?;
        document
            .analyzer_mut(analyzer_id)
            .expect("analyzer just added must exist")
            .set_name_str("Diagnostic Trace");

        let reports = document.analyze()?;
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].analyzer_name(), Some("Diagnostic Trace"));
        Ok(())
    }
}
