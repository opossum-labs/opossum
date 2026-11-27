//! Execution and orchestration of optical analyses for [`OpmDocument`].

use super::{AnalyzerInfo, OpmDocument, PositioningRun};
use crate::{
    analyzers::{AnalyzerKind, AnalyzerRegistration, AnalyzerType, raytrace::AnalysisRayTrace},
    core_optics::OpticNode,
    error::{OpmResult, OpossumError},
    light::light_result::LightResult,
    reporting::analysis_report::AnalysisReport,
};
use log::info;
use uuid::Uuid;

/// Internal description of a single planned analysis run.
#[derive(Debug, Clone)]
struct AnalysisRun {
    analyzer_id: Uuid,
    analyzer_nr: usize,
    analyzer_type: AnalyzerType,
    scenario_name: Option<String>,
    analyzer_name: Option<String>,
}

impl OpmDocument {
    /// Executes the optical simulation without generating reports or resetting node data.
    ///
    /// This is optimized for numerical inner loops (e.g. solvers, optimizers) where
    /// intermediate reports are unnecessary and detector nodes must retain their light data
    /// for direct metric querying.
    ///
    /// # Errors
    ///
    /// This function returns errors similar to the "regular" `analyze` function.
    pub fn analyze_headless(&mut self) -> OpmResult<()> {
        if self.analyzers.is_empty() {
            return Ok(());
        }
        self.metric_store.clear();
        self.vector_metric_store.clear();
        let runs = self.analysis_runs()?;
        for AnalysisRun {
            analyzer_id,
            analyzer_type,
            ..
        } in runs
        {
            let analyzer_kind: AnalyzerKind = analyzer_type.kind();
            let analyzer_box = inventory::iter::<AnalyzerRegistration>
                .into_iter()
                .find_map(|reg| (reg.builder)(&analyzer_type))
                .ok_or_else(|| {
                    OpossumError::Other(format!(
                        "No analyzer implementation found for type: {analyzer_type:?}"
                    ))
                })?;
            // 1. Remove previous ray paths from graph
            self.scenery.clear_edges();
            // 2. Execute raw physics/raytracing directly on scenery
            analyzer_box.analyze(&mut self.scenery)?;
            // 3. Collect scalar and vector observables of all detector nodes for this run
            let nodes = self.scenery.collect_all_nodes_recursive();
            for node in nodes {
                let node_id = node.uuid();
                for desc in node.observable_metrics(analyzer_kind) {
                    if let Some(val) = node.read_observable(&desc.name) {
                        self.metric_store
                            .insert((analyzer_id, node_id, desc.name.clone()), val);
                    }
                    if let Some(vec_val) = node.read_observable_vector(&desc.name)
                        && vec_val.len() > 1
                    {
                        self.vector_metric_store
                            .insert((analyzer_id, node_id, desc.name.clone()), vec_val);
                    }
                }
            }
        }
        Ok(())
    }
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
            analyzer_id: _,
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

            analyzer_box.analyze(&mut self.scenery)?;
            info!("Generating report #{analyzer_nr}");
            let mut report = analyzer_box.report(&self.scenery)?;

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

    /// Runs the positioning run on a copy of this model's scenery.
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
    /// copies every node; the rest of the document places nothing and is not copied. An edit writes
    /// a position or a property into a node without the node rebuilding its surfaces, so the copy
    /// rebuilds them first, as a ray trace does: a source then sends its axis from where it stands
    /// now.
    ///
    /// The axis leaves every source port at the port's own alignment wavelength, whatever light an
    /// analyzer sends in; the ambient medium still comes from an analyzer (see
    /// [`AnalyzerType::positioning_config`]). Unless one is named, it is the first analyzer whose
    /// light is rays, or else the first of all. Without a source port there is nothing to start the
    /// axis from, and without an analyzer no medium to send it through.
    ///
    /// # Arguments
    ///
    /// - `analyzer_id`: the analyzer to take the sources from, or `None` to follow the one chosen
    ///   as above
    ///
    /// # Returns
    ///
    /// The run: the copy with its nodes placed, the optical axis, and the connections whose
    /// distance placed a node. A run that stopped part way is returned as well; its copy and axis
    /// then give the error it stopped with.
    ///
    /// # Errors
    ///
    /// This function returns an error if the model has no source port or no analyzer, saying which
    /// it lacks, if the named analyzer does not exist, if the source ports cannot be found, or if a
    /// node cannot rebuild its surfaces.
    pub fn run_positioning(&self, analyzer_id: Option<Uuid>) -> OpmResult<PositioningRun> {
        let analyzer = self.drawing_analyzer(analyzer_id)?;
        let has_source_port = !self.scenery.find_source_ports()?.is_empty();
        let config = match (has_source_port, analyzer) {
            (true, Some((_, analyzer))) => Ok(analyzer.analyzer_type().positioning_config()),
            (false, Some(_)) => Err("no source port"),
            (true, None) => Err("no analyzer"),
            (false, None) => Err("neither"),
        };
        let mut config = config.map_err(|lacks| {
            OpossumError::OpmDocument(format!(
                "placing a model needs a source port and an analyzer defining its light, and \
                 this model has {lacks}"
            ))
        })?;
        // Only copies collect ray ends, so the scenery the copy came from holds none.
        config.set_collect_ray_ends(true);
        let mut scenery = self.scenery.clone();
        scenery.rebuild_surfaces()?;
        let outcome =
            AnalysisRayTrace::calc_node_positions(&mut scenery, LightResult::default(), &config)
                .map(|_| ());
        Ok(PositioningRun::new(scenery, outcome))
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

        for (analyzer_nr, (&analyzer_id, analyzer_info)) in self.analyzers.iter().enumerate() {
            let analyzer_name = analyzer_info.name().map(ToString::to_string);

            if analyzer_info.pump_scenarios().is_empty() {
                runs.push(AnalysisRun {
                    analyzer_id,
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
                    analyzer_id,
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
        analyzers::{AnalyzerType, RayTraceConfig, energy::EnergyConfig},
        core_optics::{NodeAttrExt, OpticNode, node_attr::NodePositioning},
        degree,
        distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
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
        nodes::{
            ALIGNMENT_WAVELENGTH, Dummy, EnergyMeter, Lens, NodeGroup, NodeReference,
            ReflectiveGrating, SourcePort,
        },
        prelude::{CollimatedSrc, RayDataSource},
        utils::{geom_transformation::Isometry, test_helper::helper::metered_energy},
    };
    use approx::assert_relative_eq;
    use uom::si::f64::Length;
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

        let run = document.run_positioning(None)?;
        let axes = run.optical_axis()?;
        assert_eq!(axes.len(), 1, "one open output, so one end");
        assert_eq!(axes[0].nr_of_rays(true), 1, "the axis is a single ray");
        let path = axes[0]
            .iter()
            .next()
            .map(Ray::position_history_with_current)
            .ok_or_else(|| OpossumError::Other("no axis ray".into()))?;

        let placed = run.positioned_scenery()?;
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

        let run = document.run_positioning(None)?;
        let lens_at = run
            .positioned_scenery()?
            .with_node_attr(lens, |attr| {
                attr.effective_position().map(Isometry::translation)
            })?
            .ok_or_else(|| OpossumError::Other("lens was not placed".into()))?;
        assert_relative_eq!(lens_at.y.value, 0.010, epsilon = 1e-12);
        assert_relative_eq!(lens_at.z.value, 0.100, epsilon = 1e-12);

        let axes = run.optical_axis()?;
        let path = axes
            .first()
            .and_then(|axis| axis.iter().next())
            .map(Ray::position_history_with_current)
            .ok_or_else(|| OpossumError::Other("no axis ray".into()))?;
        assert_relative_eq!(path[(0, 1)].value, 0.010, epsilon = 1e-12);
        Ok(())
    }

    /// A source port aligned at `port_wavelength`, a grating turned for 1053 nm and a screen
    /// behind it, with two ray trace analyzers whose light differs in wavelength and enters
    /// shifted and tilted. Returns the document, the screen and the two analyzers.
    fn a_grating_seen_by_two_analyzers(
        port_wavelength: Length,
    ) -> OpmResult<(OpmDocument, Uuid, Vec<Uuid>)> {
        let mut scenery = NodeGroup::default();
        let mut source = SourcePort::default();
        source.set_property(ALIGNMENT_WAVELENGTH, port_wavelength.into())?;
        let source = scenery.add_node(source)?;
        let grating = scenery.add_node(
            ReflectiveGrating::default()
                .with_rot_from_littrow(nanometer!(1053.0), degree!(10.0))?,
        )?;
        let screen = scenery.add_node(Dummy::new("screen"))?;
        scenery.connect_nodes(source, "output_1", grating, "input_1", millimeter!(100.0))?;
        scenery.connect_nodes(grating, "output_1", screen, "input_1", millimeter!(100.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut analyzers = Vec::new();
        for (wavelength, shift) in [(nanometer!(1053.0), 0.0), (nanometer!(1000.0), 5.0)] {
            let mut light = RayDataBuilder::from(RayDataSource::Collimated(CollimatedSrc::new(
                Hexapolar::new(millimeter!(1.0), 1)?.into(),
                UniformDist::new(joule!(1.0))?.into(),
                LaserLines::new(vec![(wavelength, 1.0)])?.into(),
            )));
            light.set_isometry(Some(Isometry::new(
                millimeter!(shift, 0.0, 0.0),
                degree!(0.0, shift, 0.0),
            )?));
            let mut config = RayTraceConfig::default();
            config.map_source(source, light);
            analyzers.push(document.add_analyzer(AnalyzerType::RayTrace(config)));
        }
        Ok((document, screen, analyzers))
    }

    /// Where `node` stands, in meters, after a positioning run that follows `analyzer`.
    fn placed_after(document: &OpmDocument, node: Uuid, analyzer: Uuid) -> OpmResult<[f64; 3]> {
        let at = document
            .run_positioning(Some(analyzer))?
            .positioned_scenery()?
            .with_node_attr(node, |attr| attr.effective_position().copied())?
            .ok_or_else(|| OpossumError::Other("the node was not placed".into()))?
            .translation();
        Ok([at.x.value, at.y.value, at.z.value])
    }

    /// The distance between two points given in meters.
    fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
        a.iter()
            .zip(b)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// The system is fixed and the analyzers examine it: two analyzers whose light differs in
    /// wavelength and in where it enters place the setup alike.
    #[test]
    fn two_analyzers_with_different_light_place_the_setup_alike() -> OpmResult<()> {
        let (document, screen, analyzers) = a_grating_seen_by_two_analyzers(nanometer!(1053.0))?;
        let first = placed_after(&document, screen, analyzers[0])?;
        let second = placed_after(&document, screen, analyzers[1])?;
        assert_relative_eq!(distance(first, second), 0.0, epsilon = 1e-12);
        Ok(())
    }

    /// The alignment wavelength of the source port is what decides where a grating sends the
    /// axis, and so where the setup behind it stands.
    #[test]
    fn the_alignment_wavelength_of_the_port_places_the_setup() -> OpmResult<()> {
        let (at_1053, screen_1053, analyzers_1053) =
            a_grating_seen_by_two_analyzers(nanometer!(1053.0))?;
        let (at_1000, screen_1000, analyzers_1000) =
            a_grating_seen_by_two_analyzers(nanometer!(1000.0))?;
        let screen_at_1053 = placed_after(&at_1053, screen_1053, analyzers_1053[0])?;
        let screen_at_1000 = placed_after(&at_1000, screen_1000, analyzers_1000[0])?;
        assert!(
            distance(screen_at_1053, screen_at_1000) > 1e-3,
            "the screen stands at {screen_at_1053:?} either way"
        );
        Ok(())
    }

    /// A positioning run tells which connections placed a node by their distance: here the one
    /// into the lens and the one into a reference to it, which an automatic source stands in front
    /// of.
    #[test]
    fn a_positioning_run_tells_which_distances_place_a_node() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let first = scenery.add_node(SourcePort::default())?;
        let lens = scenery.add_node(Lens::default())?;
        let reference = scenery.add_node(NodeReference::from_node(&scenery.node(lens)?)?)?;
        let mut second = SourcePort::default();
        second.set_positioning(NodePositioning::Automatic(None))?;
        let second = scenery.add_node(second)?;
        scenery.connect_nodes(first, "output_1", lens, "input_1", millimeter!(100.0))?;
        scenery.connect_nodes(second, "output_1", reference, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        for source in [first, second] {
            config.map_source(source, RayDataBuilder::default());
        }
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let mut placing = placing_by_distance(&document.run_positioning(None)?);
        placing.sort();
        let mut expected = vec![(first, lens), (second, reference)];
        expected.sort();
        assert_eq!(placing, expected);
        Ok(())
    }

    /// The source and target of every connection the positioning `run` lists as placing a node.
    fn placing_by_distance(run: &PositioningRun) -> Vec<(Uuid, Uuid)> {
        run.connections_placing_by_distance()
            .into_iter()
            .map(|connection| (connection.src_id, connection.target_id))
            .collect()
    }

    /// A source feeding a group whose input leads to a first dummy, connected on to a second one
    /// whose output is left open inside the group.
    ///
    /// Returns `(document, first, second)`.
    fn source_into_a_group() -> OpmResult<(OpmDocument, Uuid, Uuid)> {
        let mut group = NodeGroup::new("inner");
        let first = group.add_node(Dummy::new("first"))?;
        let second = group.add_node(Dummy::new("second"))?;
        group.connect_nodes(first, "output_1", second, "input_1", millimeter!(20.0))?;
        group.map_input_port(first, "input_1", "input_1")?;
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let group = scenery.add_node(group)?;
        scenery.connect_nodes(source, "output_1", group, "input_1", millimeter!(100.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));
        Ok((document, first, second))
    }

    /// The connections placing a node are listed for the groups nested in the model as well.
    #[test]
    fn the_distances_placing_a_node_inside_a_group_are_listed() -> OpmResult<()> {
        let (document, first, second) = source_into_a_group()?;
        let placing = placing_by_distance(&document.run_positioning(None)?);
        assert!(placing.contains(&(first, second)), "{placing:?}");
        Ok(())
    }

    /// The axis ends collected by the run, nested groups included, are handed out as the optical
    /// axis and taken out of the copy, so a trace on the copy starts without them.
    #[test]
    fn the_optical_axis_is_taken_out_of_the_positioned_copy() -> OpmResult<()> {
        let (document, _, _) = source_into_a_group()?;
        let run = document.run_positioning(None)?;
        assert_eq!(
            run.optical_axis()?.len(),
            1,
            "the open output inside the group"
        );
        assert!(run.positioned_scenery()?.ray_ends().is_empty());
        Ok(())
    }

    /// A placement that stops part way still tells which distances placed a node before it
    /// stopped: here the one into a forward reference, ahead of a component passed twice in a
    /// row, which no ray can follow.
    #[test]
    fn the_distances_placing_a_node_before_a_failure_are_still_listed() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let lens = scenery.add_node(Dummy::new("lens"))?;
        let ahead = scenery.add_node(NodeReference::from_node(&scenery.node(lens)?)?)?;
        let window = scenery.add_node(Dummy::new("window"))?;
        let again = scenery.add_node(NodeReference::from_node(&scenery.node(window)?)?)?;
        let screen = scenery.add_node(Dummy::new("screen"))?;
        scenery.connect_nodes(source, "output_1", ahead, "input_1", millimeter!(100.0))?;
        scenery.connect_nodes(ahead, "output_1", window, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(window, "output_1", again, "input_1", millimeter!(10.0))?;
        scenery.connect_nodes(again, "output_1", screen, "input_1", millimeter!(10.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));
        let run = document.run_positioning(None)?;
        assert!(run.positioned_scenery().is_err(), "the placement fails");
        assert!(run.optical_axis().is_err(), "and with it the axis");

        let placing = placing_by_distance(&run);
        assert!(placing.contains(&(source, ahead)), "{placing:?}");
        Ok(())
    }
}
