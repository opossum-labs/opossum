//! Execution and orchestration of optical analyses for [`OpmDocument`].

use super::OpmDocument;
use crate::{
    analyzers::{
        Analyzer, AnalyzerRegistration, AnalyzerType, RayTraceConfig, raytrace::AnalysisRayTrace,
    },
    core_optics::OpticNode,
    error::{OpmResult, OpossumError},
    light::{Rays, light_result::LightResult, lightdata::ray_data_builder::RayDataBuilder},
    reporting::analysis_report::AnalysisReport,
};
use log::info;
use uuid::Uuid;

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

    /// Return a copy of this document whose nodes have been placed in space.
    ///
    /// A model states what is connected to what and how far apart, not where anything is: a node
    /// only learns its own placement from a positioning run, which traces the optical axis from a
    /// source through the setup. Anything that has to draw a setup — rather than analyze it — needs
    /// that run to have happened.
    ///
    /// It happens on a copy, and that is the point of this method. A placement is written into the
    /// node itself and saved with the document, and a later run leaves an already placed node
    /// alone; running one on the live document would therefore freeze whatever the drawing happened
    /// to be asked for into the model itself. The copy is taken through the file format, which is
    /// the only copy that is genuinely independent — [`clone_deep`](OpmDocument::clone_deep) leaves
    /// the references of the copy pointing at the nodes of the original.
    ///
    /// Which source the axis starts from comes from an analyzer, because that is where source data
    /// lives; see [`AnalyzerType::positioning_config`]. A model with no analyzer that places
    /// anything is placed from its own source ports with default ray data, and one without even
    /// those gets a source port put in front of its first component, so that a setup which was
    /// never analyzed can still be looked at.
    ///
    /// # Arguments
    ///
    /// - `analyzer_id`: the analyzer to take the sources from, or `None` to pick the only one that
    ///   places anything
    ///
    /// # Returns
    ///
    /// An independent copy of this document with its nodes placed.
    ///
    /// # Errors
    ///
    /// This function returns an error if the copy cannot be made, if the named analyzer does not
    /// exist or does not place anything, if no analyzer was named and several place something, or
    /// if the positioning run itself fails.
    pub fn positioned_copy(&self, analyzer_id: Option<Uuid>) -> OpmResult<Self> {
        let mut copy = Self::from_string(&self.to_opm_file_string()?)?;
        let config = copy.placing_config(analyzer_id)?;
        AnalysisRayTrace::calc_node_positions(&mut copy.scenery, LightResult::default(), &config)?;
        copy.scenery.reset_data();
        Ok(copy)
    }

    /// Returns the paths the optical axis takes through this model when it is placed.
    ///
    /// The positioning run behind [`positioned_copy`](OpmDocument::positioned_copy) follows one
    /// ray per source — the optical axis — through the setup and places every component where
    /// that ray meets it. The ray bundles that leave the model through an open output port carry
    /// the whole path back to their source, which shows how the setup is built up. The run happens
    /// on a copy for the same reason as there, so the model itself is left untouched.
    ///
    /// # Arguments
    ///
    /// - `analyzer_id`: the analyzer to take the sources from, or `None` to pick the only one that
    ///   places anything — chosen exactly as for [`positioned_copy`](OpmDocument::positioned_copy)
    ///
    /// # Returns
    ///
    /// One bundle per open output port the axis reaches, each ray carrying its position history in
    /// global coordinates. Light that ends in a component without any output port is missing.
    ///
    /// # Errors
    ///
    /// This function returns an error in the same cases as
    /// [`positioned_copy`](OpmDocument::positioned_copy).
    pub fn positioning_rays(&self, analyzer_id: Option<Uuid>) -> OpmResult<Vec<Rays>> {
        let mut copy = Self::from_string(&self.to_opm_file_string()?)?;
        let mut config = copy.placing_config(analyzer_id)?;
        // The copy was just read from file, so it holds no ray ends yet.
        config.set_collect_ray_ends(true);
        AnalysisRayTrace::calc_node_positions(&mut copy.scenery, LightResult::default(), &config)?;
        Ok(copy.scenery.ray_ends().into_iter().cloned().collect())
    }

    /// Returns the configuration a positioning run of this model starts from.
    ///
    /// # Arguments
    ///
    /// - `analyzer_id`: the analyzer to take the sources from, or `None` to pick the only one that
    ///   places anything, falling back to default rays from the model's own source ports
    ///
    /// # Returns
    ///
    /// The ray-trace configuration whose sources the positioning run follows.
    ///
    /// # Errors
    ///
    /// This function returns an error if the named analyzer does not exist or does not place
    /// anything, if no analyzer was named and several place something, or if the default
    /// configuration cannot be built.
    fn placing_config(&mut self, analyzer_id: Option<Uuid>) -> OpmResult<RayTraceConfig> {
        if let Some(id) = analyzer_id {
            let analyzer = self.analyzer(id)?;
            return analyzer
                .analyzer_type()
                .positioning_config()
                .ok_or_else(|| {
                    OpossumError::OpmDocument(format!(
                        "the '{}' analyzer does not place the nodes of a model and cannot be used \
                         to draw it",
                        analyzer.display_name()
                    ))
                });
        }
        let mut placing = self
            .analyzers
            .values()
            .filter_map(|info| info.analyzer_type().positioning_config());
        match (placing.next(), placing.next()) {
            (Some(only), None) => Ok(only),
            (Some(_), Some(_)) => Err(OpossumError::OpmDocument(
                "this model is analyzed several ways, each of which may place its nodes \
                 differently — name the analyzer to draw it after"
                    .into(),
            )),
            // Nothing analyzes this model geometrically, so there is no source data to go by.
            // Default rays from wherever the model marks its sources still say where everything
            // sits relative to everything else.
            _ => self.default_positioning_config(),
        }
    }

    /// Build a positioning configuration for a model no analyzer places.
    ///
    /// Every source port of the model is given default ray data. A model that has none at all gets
    /// one put in front of the first component whose input is still free, at no distance, so that
    /// the setup is drawn starting from that component rather than not at all.
    ///
    /// # Returns
    ///
    /// A configuration covering every source port of the model.
    ///
    /// # Errors
    ///
    /// This function returns an error if the source ports cannot be found, or if a model that has
    /// components but no source port has nothing to put one in front of.
    fn default_positioning_config(&mut self) -> OpmResult<RayTraceConfig> {
        let mut config = RayTraceConfig::default();
        config.set_positioning_run(true);
        let mut source_ports = self.scenery.find_source_ports()?;
        // An empty model places nothing, which is not a failure — and there would be nothing to put
        // a source port in front of either.
        if source_ports.is_empty() && self.scenery.nr_of_nodes() > 0 {
            source_ports.push(self.scenery.prepend_source_port()?);
        }
        for source_port in source_ports {
            config.map_source(source_port, RayDataBuilder::default());
        }
        Ok(config)
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
        light::Ray,
        light::lightdata::energy_data_builder::{EnergyDataBuilder, EnergyLaserLines},
        millimeter, nanometer,
        nodes::{EnergyMeter, Lens, NodeGroup, SourcePort},
        utils::geom_transformation::Isometry,
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

    /// The optical axis of a source followed by two lenses, the last one's output left open.
    #[test]
    fn the_optical_axis_runs_through_every_placed_component() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let first = scenery.add_node(Lens::default())?;
        let second = scenery.add_node(Lens::default())?;
        scenery.connect_nodes(source, "output_1", first, "input_1", millimeter!(100.0))?;
        scenery.connect_nodes(first, "output_1", second, "input_1", millimeter!(50.0))?;
        let document = OpmDocument::new(scenery);

        let axes = document.positioning_rays(None)?;
        assert_eq!(axes.len(), 1, "one open output, so one end");
        assert_eq!(axes[0].nr_of_rays(true), 1, "the axis is a single ray");
        let path = axes[0]
            .iter()
            .next()
            .map(Ray::position_history_with_current)
            .ok_or_else(|| OpossumError::Other("no axis ray".into()))?;

        let placed = document.positioned_copy(None)?;
        for lens in [first, second] {
            let position = placed
                .scenery()
                .with_node_attr(lens, |attr| {
                    attr.effective_position().map(Isometry::translation)
                })?
                .ok_or_else(|| OpossumError::Other("lens was not placed".into()))?;
            let on_path = path.row_iter().any(|point| {
                (point[0] - position.x).abs() < millimeter!(1e-6)
                    && (point[1] - position.y).abs() < millimeter!(1e-6)
                    && (point[2] - position.z).abs() < millimeter!(1e-6)
            });
            assert!(on_path, "the axis passes the lens at {position:?}");
        }
        assert!(
            document
                .scenery()
                .with_node_attr(first, |attr| attr.effective_position().is_none())?,
            "the model itself stays unplaced"
        );
        Ok(())
    }
}
