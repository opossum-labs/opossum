//! Execution and orchestration of optical analyses for [`OpmDocument`].

use super::{AnalyzerInfo, OpmDocument};
use crate::{
    analyzers::{
        Analyzer, AnalyzerRegistration, AnalyzerType, RayTraceConfig,
        raytrace::{AnalysisRayTrace, RayTracingAnalyzer},
    },
    core_optics::OpticNode,
    error::{OpmResult, OpossumError},
    light::{Rays, light_result::LightResult},
    nodes::NodeGroup,
    reporting::analysis_report::AnalysisReport,
};
use log::info;
use std::ops::Range;
use uom::si::f64::Length;
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
        let config = self.placing_config(analyzer_id)?;
        let mut scenery = self.scenery.clone();
        AnalysisRayTrace::calc_node_positions(&mut scenery, LightResult::default(), &config)?;
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
        let mut scenery = self.scenery.clone();
        // Only copies collect ray ends, so the scenery the copy came from holds none.
        config.set_collect_ray_ends(true);
        AnalysisRayTrace::calc_node_positions(&mut scenery, LightResult::default(), &config)?;
        Ok(scenery.ray_ends().into_iter().cloned().collect())
    }

    /// Returns the rays of the model's sources, traced through it, for drawing them.
    ///
    /// The sources are the ones the analyzer defines, and the model is placed exactly as
    /// [`placed_scenery`](OpmDocument::placed_scenery) places it, so the rays run through the
    /// components where the 3D view shows them. The sources share `max_rays` evenly, and each is
    /// coarsened to its share first, every ray shining one wavelength of its spectrum (see
    /// [`RayDataSource::coarsened`](crate::light::lightdata::ray_data_source::RayDataSource::coarsened)).
    /// That keeps the trace cheap however many rays and spectral samples the sources hold, while
    /// every wavelength is still traced. Only the primary path is traced: a ghost focus analyzer
    /// contributes its sources, not its reflections. As everywhere else, this runs on a copy and
    /// leaves the model untouched.
    ///
    /// # Arguments
    ///
    /// - `analyzer_id`: the analyzer to take the sources from, or `None` to follow the one chosen
    ///   exactly as for [`placed_scenery`](OpmDocument::placed_scenery)
    /// - `max_rays`: the most rays traced from all sources together; each source keeps at least one
    ///
    /// # Returns
    ///
    /// One bundle per open output port the light reaches, each ray carrying its path in global
    /// coordinates; rays stopped on the way are included up to where they stopped. Empty if the
    /// model has no analyzer, or if the one followed has no rays of its own, as an energy analysis:
    /// default rays would only show something that is not the setup's own light.
    ///
    /// # Errors
    ///
    /// This function returns an error if the named analyzer does not exist, if the positioning run
    /// fails, or if a source cannot be built or the trace fails.
    pub fn traced_rays(&self, analyzer_id: Option<Uuid>, max_rays: usize) -> OpmResult<Vec<Rays>> {
        let Some((_, analyzer)) = self
            .drawing_analyzer(analyzer_id)?
            .filter(|(_, analyzer)| analyzer.analyzer_type().has_ray_sources())
        else {
            return Ok(Vec::new());
        };
        let placing = analyzer.analyzer_type().positioning_config();
        let mut scenery = self.scenery.clone();
        // Placed from the full sources first, exactly as the components are drawn: the trace below
        // leaves placed nodes where they are, so the coarsened sources cannot move anything.
        AnalysisRayTrace::calc_node_positions(&mut scenery, LightResult::default(), &placing)?;
        scenery.reset_data();

        let mut config = placing;
        config.set_positioning_run(false);
        // Only copies collect ray ends, so the scenery the copy came from holds none.
        config.set_collect_ray_ends(true);
        let per_source = max_rays / config.source_map().len().max(1);
        // A coarsened source keeps its builder's isometry and is aligned like the full one.
        let coarsened = config
            .source_map()
            .iter()
            .map(|(source, builder)| {
                let mut coarse = builder.clone();
                coarse.set_alignment_wavelength(Some(builder.axis_wavelength()?));
                coarse.set_source(builder.source().coarsened(per_source)?);
                Ok((*source, coarse))
            })
            .collect::<OpmResult<_>>()?;
        config.set_source_map(coarsened);
        RayTracingAnalyzer::new(config).analyze(&mut scenery)?;
        Ok(scenery.ray_ends().into_iter().cloned().collect())
    }

    /// Returns the range of wavelengths the sources of [`traced_rays`](OpmDocument::traced_rays)
    /// emit, without building any rays.
    ///
    /// # Arguments
    ///
    /// - `analyzer_id`: the analyzer to take the sources from, or `None` to follow the one chosen
    ///   exactly as for [`placed_scenery`](OpmDocument::placed_scenery)
    ///
    /// # Returns
    ///
    /// The shortest and longest wavelength of all those sources, the same for light of one
    /// wavelength. `None` if there are no rays to draw, as for [`traced_rays`](OpmDocument::traced_rays).
    ///
    /// # Errors
    ///
    /// This function returns an error if the named analyzer does not exist or a spectrum cannot be
    /// sampled.
    pub fn source_wavelength_range(
        &self,
        analyzer_id: Option<Uuid>,
    ) -> OpmResult<Option<Range<Length>>> {
        let Some((_, analyzer)) = self
            .drawing_analyzer(analyzer_id)?
            .filter(|(_, analyzer)| analyzer.analyzer_type().has_ray_sources())
        else {
            return Ok(None);
        };
        let mut range: Option<Range<Length>> = None;
        // A range does not depend on the order the sources are visited in.
        for builder in analyzer
            .analyzer_type()
            .positioning_config()
            .source_map()
            .values()
        {
            for wavelength in builder.source().wavelengths()? {
                range = Some(range.map_or(wavelength..wavelength, |range| {
                    range.start.min(wavelength)..range.end.max(wavelength)
                }));
            }
        }
        Ok(range)
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
        apertures::{CircleShape, OpticShape},
        core_optics::NodeAttrExt,
        degree,
        distributions::{
            energy::UniformDist,
            position::Hexapolar,
            spectral::{Gaussian, LaserLines, SpecDistType},
        },
        gain::{ConstGain, GainModel},
        geometry::body::OPTIC_SHAPE,
        joule,
        light::{
            Ray,
            lightdata::{
                energy_data_builder::{EnergyDataBuilder, EnergyLaserLines},
                ray_data_builder::RayDataBuilder,
                ray_data_source::{CollimatedSrc, RayDataSource},
            },
        },
        millimeter, nanometer,
        nodes::{
            Dummy, EnergyMeter, Lens, NodeGroup, ReflectiveGrating, SourcePort, ThinMirror,
            round_collimated_ray_builder,
        },
        utils::{geom_transformation::Isometry, test_helper::helper::metered_energy},
    };
    use approx::assert_relative_eq;
    use num_traits::Zero;
    use uom::si::{f64::Length, length::millimeter};
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

    /// Without an analyzer there is no light of the setup's own to show.
    #[test]
    fn a_model_nobody_analyzes_traces_no_rays() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let lens = scenery.add_node(Lens::default())?;
        scenery.connect_nodes(source, "output_1", lens, "input_1", millimeter!(100.0))?;
        assert!(OpmDocument::new(scenery).traced_rays(None, 100)?.is_empty());
        Ok(())
    }

    /// A collimated source of 1 J and 1 mm radius with the given hexapolar rings and spectrum.
    fn collimated_source(nr_of_rings: u8, spectrum: SpecDistType) -> OpmResult<RayDataBuilder> {
        Ok(RayDataSource::Collimated(CollimatedSrc::new(
            Hexapolar::new(millimeter!(1.0), nr_of_rings)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            spectrum,
        ))
        .into())
    }

    /// A model tracing the given source into a dummy whose output stays open.
    fn source_then_open_dummy(builder: RayDataBuilder) -> OpmResult<OpmDocument> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let dummy = scenery.add_node(Dummy::default())?;
        scenery.connect_nodes(source, "output_1", dummy, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, builder);
        document.add_analyzer(AnalyzerType::RayTrace(config));
        Ok(document)
    }

    /// A source of 721 rays is traced with at most the asked number, which still cover all of it:
    /// the outermost one starts on its rim.
    #[test]
    fn the_traced_rays_of_a_source_cover_all_of_it() -> OpmResult<()> {
        let document = source_then_open_dummy(round_collimated_ray_builder(
            millimeter!(5.0),
            joule!(1.0),
            15,
        )?)?;

        let ends = document.traced_rays(None, 100)?;

        assert_eq!(ends.len(), 1, "one open output");
        assert!(ends[0].nr_of_rays(false) <= 100);
        let outermost = ends[0]
            .iter()
            .filter_map(|ray| {
                let path = ray.position_history_with_current();
                let start = path.row_iter().next()?;
                Some((start[0] * start[0] + start[1] * start[1]).sqrt())
            })
            .fold(Length::zero(), Length::max);
        assert_relative_eq!(outermost.get::<millimeter>(), 5.0, epsilon = 1e-9);
        Ok(())
    }

    /// Ten laser lines of equal power, 10 nm apart from 1000 nm on.
    fn ten_lines() -> OpmResult<SpecDistType> {
        let lines = (0..10)
            .map(|i| (nanometer!(10.0f64.mul_add(f64::from(i), 1000.0)), 1.0))
            .collect();
        Ok(LaserLines::new(lines)?.into())
    }

    /// Every laser line of a source is traced, with at most the asked number of rays for all of
    /// them together. Thinning the built rays used to keep the first line only: all lines of a
    /// point start there.
    #[test]
    fn every_wavelength_of_a_source_is_traced() -> OpmResult<()> {
        let document = source_then_open_dummy(collimated_source(15, ten_lines()?)?)?;

        let ends = document.traced_rays(None, 100)?;

        assert_eq!(ends.len(), 1, "one open output");
        assert!(ends[0].nr_of_rays(false) <= 100);
        let (lines, _) = ends[0].split_ray_bundle_by_wavelength(nanometer!(1.0), true)?;
        assert_eq!(lines.len(), 10, "every line arrives");
        Ok(())
    }

    /// The asked number of rays is shared by all sources, and each of them is drawn.
    #[test]
    fn max_rays_counts_the_rays_of_all_sources_together() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let mut config = RayTraceConfig::default();
        for _ in 0..2 {
            let source = scenery.add_node(SourcePort::default())?;
            let dummy = scenery.add_node(Dummy::default())?;
            scenery.connect_nodes(source, "output_1", dummy, "input_1", millimeter!(50.0))?;
            config.map_source(source, collimated_source(15, ten_lines()?)?);
        }
        let mut document = OpmDocument::new(scenery);
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let ends = document.traced_rays(None, 100)?;

        assert_eq!(ends.len(), 2, "one open output per source");
        assert!(ends.iter().all(|end| end.nr_of_rays(false) > 0));
        let total: usize = ends.iter().map(|end| end.nr_of_rays(false)).sum();
        assert!(total <= 100, "{total} rays");
        Ok(())
    }

    /// A spectrum through a grating set to Littrow for its center, on to a mirror with a component
    /// behind it. The trace re-runs the positioning along the sources it traces, so it has to find
    /// the axis the model was placed by. Thinning used to leave only the shortest wavelength, whose
    /// axis left the grating at another angle and missed the mirror.
    #[test]
    fn a_spectrum_behind_a_grating_is_traced_to_the_end() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let grating = scenery.add_node(
            ReflectiveGrating::default().with_rot_from_littrow(nanometer!(1053.0), degree!(0.0))?,
        )?;
        let mirror = scenery.add_node(ThinMirror::default())?;
        let screen = scenery.add_node(Dummy::default())?;
        scenery.connect_nodes(source, "output_1", grating, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(grating, "output_1", mirror, "input_1", millimeter!(200.0))?;
        scenery.connect_nodes(mirror, "output_1", screen, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let spectrum = Gaussian::new(
            (nanometer!(1003.0), nanometer!(1103.0)),
            50,
            nanometer!(1053.0),
            nanometer!(10.0),
            1.0,
        )?;
        let mut config = RayTraceConfig::default();
        config.map_source(source, collimated_source(7, spectrum.into())?);
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let ends = document.traced_rays(None, 100)?;

        assert_eq!(
            ends.len(),
            1,
            "the light reaches the open output behind the mirror"
        );
        Ok(())
    }

    /// Two lines either side of a grating's Littrow wavelength, few rays, and a mirror far behind
    /// it. The trace re-runs the positioning, so the coarsened source must put its axis where the
    /// full one did: a few nanometers off, it would leave the grating beside the mirror.
    #[test]
    fn a_coarsened_source_is_aligned_like_the_full_one() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let grating = scenery.add_node(
            ReflectiveGrating::default().with_rot_from_littrow(nanometer!(1050.0), degree!(0.0))?,
        )?;
        let mirror = scenery.add_node(ThinMirror::default())?;
        let screen = scenery.add_node(Dummy::default())?;
        scenery.connect_nodes(source, "output_1", grating, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(grating, "output_1", mirror, "input_1", millimeter!(2000.0))?;
        scenery.connect_nodes(mirror, "output_1", screen, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let lines = LaserLines::new(vec![(nanometer!(1000.0), 1.0), (nanometer!(1100.0), 1.0)])?;
        let mut config = RayTraceConfig::default();
        config.map_source(source, collimated_source(1, lines.into())?);
        document.add_analyzer(AnalyzerType::RayTrace(config));

        document.traced_rays(None, 7)?;
        Ok(())
    }

    /// The range of wavelengths drawn is that of the sources' spectra, read without building any
    /// rays: from the first to the last sample of a spectrum, a single line for a laser.
    #[test]
    fn the_wavelengths_of_the_sources_span_their_spectra() -> OpmResult<()> {
        let gaussian = Gaussian::new(
            (nanometer!(1003.0), nanometer!(1103.0)),
            50,
            nanometer!(1053.0),
            nanometer!(10.0),
            1.0,
        )?;
        let lines = LaserLines::new(vec![(nanometer!(1053.0), 1.0)])?;
        for (spectrum, range) in [
            (
                SpecDistType::from(gaussian),
                nanometer!(1003.0)..nanometer!(1103.0),
            ),
            (lines.into(), nanometer!(1053.0)..nanometer!(1053.0)),
        ] {
            let document = source_then_open_dummy(collimated_source(1, spectrum)?)?;
            let drawn = document
                .source_wavelength_range(None)?
                .ok_or_else(|| OpossumError::Other("no range".into()))?;
            assert_relative_eq!(drawn.start.value, range.start.value, max_relative = 1e-12);
            assert_relative_eq!(drawn.end.value, range.end.value, max_relative = 1e-12);
        }
        Ok(())
    }

    /// The range spans every source of the analyzer.
    #[test]
    fn the_wavelengths_of_several_sources_are_spanned_together() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let first = scenery.add_node(SourcePort::default())?;
        let second = scenery.add_node(SourcePort::default())?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        for (source, nm) in [(first, 527.0), (second, 1053.0)] {
            let line = LaserLines::new(vec![(nanometer!(nm), 1.0)])?;
            config.map_source(source, collimated_source(1, line.into())?);
        }
        document.add_analyzer(AnalyzerType::RayTrace(config));

        assert_eq!(
            document.source_wavelength_range(None)?,
            Some(nanometer!(527.0)..nanometer!(1053.0))
        );
        Ok(())
    }

    /// An energy analysis has no rays to draw, so no wavelengths to draw them in.
    #[test]
    fn an_energy_analysis_has_no_wavelengths_to_draw() -> OpmResult<()> {
        let (document, _) = document_with_one_analyzer()?;
        assert_eq!(document.source_wavelength_range(None)?, None);
        Ok(())
    }

    /// A source of 721 rays behind an aperture that only lets the core through: at most the
    /// asked number is traced, and the rays the aperture stops end there.
    #[test]
    #[ignore = "a ray beyond a clear aperture is stopped where it last was, not at the aperture"]
    fn the_traced_rays_are_coarsened_and_end_where_they_are_stopped() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let mut aperture = Dummy::default();
        aperture.set_property(
            OPTIC_SHAPE,
            OpticShape::from(CircleShape::new(millimeter!(2.0))?).into(),
        )?;
        let aperture = scenery.add_node(aperture)?;
        let screen = scenery.add_node(Dummy::default())?;
        scenery.connect_nodes(source, "output_1", aperture, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(aperture, "output_1", screen, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(5.0), joule!(1.0), 15)?,
        );
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let ends = document.traced_rays(None, 100)?;
        assert_eq!(ends.len(), 1, "one open output");
        let rays = &ends[0];
        assert!(rays.nr_of_rays(false) <= 100);
        // How far along the axis the rays that were or were not stopped got, at most.
        let reach = |valid: bool| {
            rays.iter()
                .filter(|ray| ray.valid() == valid)
                .map(|ray| ray.position().z)
                .fold(None, |far: Option<Length>, z| {
                    Some(far.map_or(z, |far| far.max(z)))
                })
        };
        let stopped = reach(false).expect("the aperture stops the outer rays");
        let passed = reach(true).expect("the aperture passes the core");
        assert_relative_eq!(stopped.get::<millimeter>(), 50.0, epsilon = 1e-9);
        assert_relative_eq!(passed.get::<millimeter>(), 100.0, epsilon = 1e-9);
        Ok(())
    }
}
