//! Execution and orchestration of optical analyses for [`OpmDocument`].

use super::{AnalyzerInfo, OpmDocument};
use crate::{
    analyzers::{
        Analyzer, AnalyzerRegistration, AnalyzerType, RayTraceConfig, raytrace::AnalysisRayTrace,
    },
    core_optics::OpticNode,
    error::{OpmResult, OpossumError},
    light::{Rays, light_result::LightResult},
    nodes::{ConnectionInfo, NodeGroup},
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

    /// Return a copy of this model's scenery whose nodes have been placed in space.
    ///
    /// A model states what is connected to what and how far apart, not where anything is: a node
    /// only learns its own placement from a positioning run, which traces the optical axis from a
    /// source through the setup. Anything that has to draw a setup — rather than analyze it — needs
    /// that run to have happened.
    ///
    /// It happens on a copy, and that is the point of this method. A placement is written into the
    /// node itself and saved with the document, and a later run leaves an already placed node
    /// alone; running one on the live document would therefore freeze whatever the drawing happened
    /// to be asked for into the model itself. The copy is a plain [`Clone`] of the scenery, which
    /// copies every node; the rest of the document places nothing and is not copied.
    ///
    /// Which source the axis starts from comes from an analyzer, because that is where source data
    /// lives; see [`AnalyzerType::positioning_config`]. Unless one is named, it is the first
    /// analyzer whose light is rays, or else the first of all. Without a source port, or without an
    /// analyzer to define its light, there is nothing to start the axis from.
    ///
    /// # Arguments
    ///
    /// - `analyzer_id`: the analyzer to take the sources from, or `None` to follow the one chosen
    ///   as above
    ///
    /// # Returns
    ///
    /// An independent copy of this model's scenery with its nodes placed.
    ///
    /// # Errors
    ///
    /// This function returns an error if the model has no source port or no analyzer, if the
    /// named analyzer does not exist, or if the positioning run itself fails.
    pub fn placed_scenery(&self, analyzer_id: Option<Uuid>) -> OpmResult<NodeGroup> {
        let mut scenery = self.placed_copy(&self.placing_config(analyzer_id)?)?;
        scenery.reset_data();
        Ok(scenery)
    }

    /// Returns the paths the optical axis takes through this model when it is placed.
    ///
    /// The positioning run behind [`placed_scenery`](OpmDocument::placed_scenery) follows one
    /// ray per source — the optical axis — through the setup and places every component where
    /// that ray meets it. The ray bundles that leave the model through an open output port carry
    /// the whole path back to their source, which shows how the setup is built up. The run happens
    /// on a copy for the same reason as there, so the model itself is left untouched.
    ///
    /// # Arguments
    ///
    /// - `analyzer_id`: the analyzer to take the sources from, or `None` to follow the one chosen
    ///   exactly as for [`placed_scenery`](OpmDocument::placed_scenery)
    ///
    /// # Returns
    ///
    /// One bundle per open output port the axis reaches, each ray carrying its position history in
    /// global coordinates. Light that ends in a component without any output port is missing.
    ///
    /// # Errors
    ///
    /// This function returns an error in the same cases as
    /// [`placed_scenery`](OpmDocument::placed_scenery).
    pub fn positioning_rays(&self, analyzer_id: Option<Uuid>) -> OpmResult<Vec<Rays>> {
        let mut config = self.placing_config(analyzer_id)?;
        // Only copies collect ray ends, so the scenery the copy came from holds none.
        config.set_collect_ray_ends(true);
        Ok(self
            .placed_copy(&config)?
            .ray_ends()
            .into_iter()
            .cloned()
            .collect())
    }

    /// Returns the connections of a group of this model whose distance places a node when the
    /// model is placed.
    ///
    /// A distance places a node when the node it leads to is placed along it, or when the node it
    /// comes from stands in front of where it leads. A distance into a node placed absolutely, or
    /// into a reference taking the placement of the node it refers to, places nothing. The model
    /// is placed on a copy, as for [`placed_scenery`](OpmDocument::placed_scenery).
    ///
    /// # Arguments
    ///
    /// - `analyzer_id`: the analyzer to take the sources from, or `None` to follow the one chosen
    ///   as for [`placed_scenery`](OpmDocument::placed_scenery)
    /// - `group_id`: the group whose connections to look at; the scenery itself is a group too
    ///
    /// # Returns
    ///
    /// The connections of that group, not of the groups nested in it, whose distance places a
    /// node.
    ///
    /// # Errors
    ///
    /// This function returns an error in the same cases as
    /// [`placed_scenery`](OpmDocument::placed_scenery), or if the model has no group `group_id`.
    pub fn placing_connections(
        &self,
        analyzer_id: Option<Uuid>,
        group_id: Uuid,
    ) -> OpmResult<Vec<ConnectionInfo>> {
        self.placed_copy(&self.placing_config(analyzer_id)?)?
            .with_group_node(group_id, |group| group.graph().placing_connections())
    }

    /// Returns a copy of this model's scenery, placed by a positioning run.
    ///
    /// An edit writes a position or a property into a node without the node rebuilding its
    /// surfaces, so the copy rebuilds them first, as a ray trace does: a source then sends its axis
    /// from where it stands now.
    ///
    /// # Arguments
    ///
    /// - `config`: the configuration of the positioning run
    ///
    /// # Returns
    ///
    /// The placed copy, with the data the run left on it.
    ///
    /// # Errors
    ///
    /// This function returns an error if a node cannot rebuild its surfaces or if the positioning
    /// run fails.
    fn placed_copy(&self, config: &RayTraceConfig) -> OpmResult<NodeGroup> {
        let mut scenery = self.scenery.clone();
        scenery.rebuild_surfaces()?;
        AnalysisRayTrace::calc_node_positions(&mut scenery, LightResult::default(), config)?;
        Ok(scenery)
    }

    /// Returns the configuration a positioning run of this model starts from.
    ///
    /// # Arguments
    ///
    /// - `analyzer_id`: the analyzer to take the sources from, or `None` to follow the one chosen
    ///   as for [`placed_scenery`](OpmDocument::placed_scenery)
    ///
    /// # Returns
    ///
    /// The ray-trace configuration whose sources the positioning run follows.
    ///
    /// # Errors
    ///
    /// This function returns an error if the model has no source port or no analyzer, saying which
    /// it lacks, if the named analyzer does not exist, or if the source ports cannot be found.
    fn placing_config(&self, analyzer_id: Option<Uuid>) -> OpmResult<RayTraceConfig> {
        let analyzer = self.drawing_analyzer(analyzer_id)?;
        let has_source_port = !self.scenery.find_source_ports()?.is_empty();
        let lacks = match (has_source_port, analyzer) {
            (true, Some((_, analyzer))) => {
                return Ok(analyzer.analyzer_type().positioning_config());
            }
            (false, Some(_)) => "no source port",
            (true, None) => "no analyzer",
            (false, None) => "neither",
        };
        Err(OpossumError::OpmDocument(format!(
            "placing a model needs a source port and an analyzer defining its light, and this \
             model has {lacks}"
        )))
    }

    /// Returns the analyzer a drawing of this model follows.
    ///
    /// Two analyses can place the same model differently — another source, another alignment
    /// wavelength — so a drawing follows the one named. Without a name it follows the first
    /// analysis whose light is rays, as that one can show its light as well, and the first of all
    /// only when none has rays.
    ///
    /// # Arguments
    ///
    /// - `analyzer_id`: the analyzer to follow, or `None` to choose one as above
    ///
    /// # Returns
    ///
    /// The analyzer with its [`Uuid`], or `None` if the model has none.
    ///
    /// # Errors
    ///
    /// This function returns an error if the named analyzer does not exist.
    pub fn drawing_analyzer(
        &self,
        analyzer_id: Option<Uuid>,
    ) -> OpmResult<Option<(Uuid, &AnalyzerInfo)>> {
        let Some(id) = analyzer_id else {
            return Ok(self
                .analyzers
                .iter()
                .find(|(_, analyzer)| analyzer.analyzer_type().has_ray_sources())
                .or_else(|| self.analyzers.first())
                .map(|(id, analyzer)| (*id, analyzer)));
        };
        self.analyzers
            .get(&id)
            .map(|analyzer| Some((id, analyzer)))
            .ok_or_else(|| {
                OpossumError::OpmDocument(format!(
                    "this model has no analyzer {id} to draw it after"
                ))
            })
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
        core_optics::{
            OpticNode,
            node_attr::{HasNodeAttr, NodePositioning},
        },
        degree,
        gain::{ConstGain, GainModel},
        joule,
        light::{
            Ray,
            lightdata::{
                energy_data_builder::{EnergyDataBuilder, EnergyLaserLines},
                ray_data_builder::RayDataBuilder,
            },
        },
        millimeter, nanometer,
        nodes::{EnergyMeter, Lens, NodeGroup, NodeReference, SourcePort},
        utils::{geom_transformation::Isometry, test_helper::helper::metered_energy},
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
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let axes = document.positioning_rays(None)?;
        assert_eq!(axes.len(), 1, "one open output, so one end");
        assert_eq!(axes[0].nr_of_rays(true), 1, "the axis is a single ray");
        let path = axes[0]
            .iter()
            .next()
            .map(Ray::position_history_with_current)
            .ok_or_else(|| OpossumError::Other("no axis ray".into()))?;

        let placed = document.placed_scenery(None)?;
        for lens in [first, second] {
            let position = placed
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

    /// An edit moves a source the way the backend does: it writes the new position into the
    /// source's attributes, and the source does not rebuild its surfaces. The model is placed, and
    /// its optical axis starts, from where the source stands now.
    #[test]
    fn a_source_moved_by_an_edit_places_the_model_from_where_it_stands_now() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let lens = scenery.add_node(Lens::default())?;
        scenery.connect_nodes(source, "output_1", lens, "input_1", millimeter!(100.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));
        let moved = Isometry::new(millimeter!(0.0, 10.0, 0.0), degree!(0.0, 0.0, 0.0))?;
        document.scenery_mut().with_node_attr_mut(source, |attr| {
            attr.set_positioning(NodePositioning::Absolute(moved));
        })?;

        let lens_at = document
            .placed_scenery(None)?
            .with_node_attr(lens, |attr| {
                attr.effective_position().map(Isometry::translation)
            })?
            .ok_or_else(|| OpossumError::Other("lens was not placed".into()))?;
        assert_relative_eq!(lens_at.y.value, 0.010, epsilon = 1e-12);
        assert_relative_eq!(lens_at.z.value, 0.100, epsilon = 1e-12);

        let axes = document.positioning_rays(None)?;
        let path = axes
            .first()
            .and_then(|axis| axis.iter().next())
            .map(Ray::position_history_with_current)
            .ok_or_else(|| OpossumError::Other("no axis ray".into()))?;
        assert_relative_eq!(path[(0, 1)].value, 0.010, epsilon = 1e-12);
        Ok(())
    }

    /// A model asked for the connections of one of its groups whose distance places a node, when
    /// the model is placed: here the one into the lens and the one into a reference to it, which
    /// an automatic source stands in front of. A group it does not have is an error.
    #[test]
    fn a_model_tells_which_distances_of_a_group_place_a_node() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let first = scenery.add_node(SourcePort::default())?;
        let lens = scenery.add_node(Lens::default())?;
        let reference = scenery.add_node(NodeReference::from_node(&scenery.node(lens)?)?)?;
        let mut second = SourcePort::default();
        second.set_positioning(NodePositioning::Automatic(None))?;
        let second = scenery.add_node(second)?;
        scenery.connect_nodes(first, "output_1", lens, "input_1", millimeter!(100.0))?;
        scenery.connect_nodes(second, "output_1", reference, "input_1", millimeter!(50.0))?;
        let root = scenery.node_attr().uuid();
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        for source in [first, second] {
            config.map_source(source, RayDataBuilder::default());
        }
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let mut placing: Vec<(Uuid, Uuid)> = document
            .placing_connections(None, root)?
            .into_iter()
            .map(|connection| (connection.src_id, connection.target_id))
            .collect();
        placing.sort();
        let mut expected = vec![(first, lens), (second, reference)];
        expected.sort();
        assert_eq!(placing, expected);
        assert!(document.placing_connections(None, Uuid::new_v4()).is_err());
        Ok(())
    }
}
