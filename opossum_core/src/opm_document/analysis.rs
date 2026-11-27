//! Execution and orchestration of optical analyses for [`OpmDocument`].

use super::{AnalyzerInfo, OpmDocument, PositioningRun};
use crate::{
    analyzers::{
        AnalyzerKind, AnalyzerRegistration, AnalyzerType, RayTraceConfig,
        raytrace::AnalysisRayTrace,
    },
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
    /// The placement is the setup's own, the same whichever analyzer examines it: see
    /// [`positioning_config`](Self::positioning_config). Without a source port there is nothing
    /// to start the optical axis from.
    ///
    /// # Returns
    ///
    /// The run: the copy with its nodes placed, the optical axis, and the connections whose
    /// distance placed a node. A run that stopped part way is returned as well; its copy and axis
    /// then give the error it stopped with.
    ///
    /// # Errors
    ///
    /// This function returns an error if the model has no source port, if the source ports cannot
    /// be found, or if a node cannot rebuild its surfaces.
    pub fn run_positioning(&self) -> OpmResult<PositioningRun> {
        if self.scenery.find_source_ports()?.is_empty() {
            return Err(OpossumError::OpmDocument(
                "placing a model needs a source port to start the optical axis from, and this \
                 model has no source port"
                    .into(),
            ));
        }
        let mut config = self.positioning_config();
        // Only copies collect ray ends, so the scenery the copy came from holds none.
        config.set_collect_ray_ends(true);
        let mut scenery = self.scenery.clone();
        scenery.rebuild_surfaces()?;
        let outcome =
            AnalysisRayTrace::calc_node_positions(&mut scenery, LightResult::default(), &config)
                .map(|_| ());
        Ok(PositioningRun::new(scenery, outcome))
    }

    /// Returns the configuration this model is placed with.
    ///
    /// The system is fixed and the analyzers examine it, so the placement depends on the setup
    /// alone: each source port sends the optical axis at its own alignment wavelength (see
    /// [`SourcePort`](crate::nodes::SourcePort)), through the medium the setup is aligned in (see
    /// [`set_ambient_material`](Self::set_ambient_material)). The light and the medium an analyzer
    /// defines play no part.
    ///
    /// # Returns
    ///
    /// The configuration for the positioning run.
    #[must_use]
    pub fn positioning_config(&self) -> RayTraceConfig {
        let mut config = RayTraceConfig::default();
        config.set_ambient_material_ref(self.ambient_material.clone());
        config.for_positioning()
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
        core_optics::{Alignable, NodeAttrExt, OpticNode, OpticRef, node_attr::NodePositioning},
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
        material::Material,
        millimeter, nanometer,
        nodes::{
            ALIGNMENT_WAVELENGTH, Dummy, EnergyMeter, Lens, NodeGroup, NodeReference,
            ReflectiveGrating, SourcePort,
        },
        prelude::{CollimatedSrc, RayDataSource},
        refractive_index::RefrIndexConst,
        utils::{geom_transformation::Isometry, test_helper::helper::metered_energy},
    };
    use approx::assert_relative_eq;
    use nalgebra::Point3;
    use num_traits::Zero;
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

        let run = document.run_positioning()?;
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

        let run = document.run_positioning()?;
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

    /// A source port aligned at `port_wavelength`, `turning` 100 mm after it and a screen 100 mm
    /// after that, analyzed by a ray trace whose light shines at `light_wavelength`, enters
    /// shifted and tilted by `shift` (in millimeters and degrees) and runs through `medium`.
    /// Returns the document and the screen.
    fn a_setup_analyzed_with(
        turning: OpticRef,
        port_wavelength: Length,
        light_wavelength: Length,
        shift: f64,
        medium: Option<Material>,
    ) -> OpmResult<(OpmDocument, Uuid)> {
        let mut scenery = NodeGroup::default();
        let mut source = SourcePort::default();
        source.set_property(ALIGNMENT_WAVELENGTH, port_wavelength.into())?;
        let source = scenery.add_node(source)?;
        let turning = scenery.add_node_ref(turning)?;
        let screen = scenery.add_node(Dummy::new("screen"))?;
        scenery.connect_nodes(source, "output_1", turning, "input_1", millimeter!(100.0))?;
        scenery.connect_nodes(turning, "output_1", screen, "input_1", millimeter!(100.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut light = RayDataBuilder::from(RayDataSource::Collimated(CollimatedSrc::new(
            Hexapolar::new(millimeter!(1.0), 1)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(vec![(light_wavelength, 1.0)])?.into(),
        )));
        light.set_isometry(Some(Isometry::new(
            millimeter!(shift, 0.0, 0.0),
            degree!(0.0, shift, 0.0),
        )?));
        let mut config = RayTraceConfig::default();
        config.map_source(source, light);
        if let Some(medium) = medium {
            config.set_ambient_material(medium);
        }
        document.add_analyzer(AnalyzerType::RayTrace(config));
        Ok((document, screen))
    }

    /// A grating turned for 1053 nm: it sends the axis off by wavelength.
    fn grating() -> OpmResult<OpticRef> {
        Ok(OpticRef::new(Box::new(
            ReflectiveGrating::default()
                .with_rot_from_littrow(nanometer!(1053.0), degree!(10.0))?,
        )))
    }

    /// A lens 10 mm off the axis, within its clear aperture: it sends the axis off by the medium
    /// around it.
    fn decentered_lens() -> OpmResult<OpticRef> {
        Ok(OpticRef::new(Box::new(Lens::default().with_decenter(
            Point3::new(Length::zero(), millimeter!(10.0), Length::zero()),
        )?)))
    }

    /// A medium of constant refractive index 1.33.
    fn water() -> OpmResult<Material> {
        Ok(Material::new_draft(
            "water",
            None,
            None,
            RefrIndexConst::new(1.33)?.into(),
        ))
    }

    /// Where the screen stands, in meters, once the model is placed.
    fn screen_placed(document: &OpmDocument, screen: Uuid) -> OpmResult<[f64; 3]> {
        let at = document
            .run_positioning()?
            .positioned_scenery()?
            .with_node_attr(screen, |attr| attr.effective_position().copied())?
            .ok_or_else(|| OpossumError::Other("the screen was not placed".into()))?
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

    /// The system is fixed and the analyzers examine it: analyses whose light differs in
    /// wavelength and in where it enters place the setup alike.
    #[test]
    fn analyses_with_different_light_place_the_setup_alike() -> OpmResult<()> {
        let at_1053 = nanometer!(1053.0);
        let (first, screen) = a_setup_analyzed_with(grating()?, at_1053, at_1053, 0.0, None)?;
        let (second, other_screen) =
            a_setup_analyzed_with(grating()?, at_1053, nanometer!(1000.0), 5.0, None)?;
        assert_relative_eq!(
            distance(
                screen_placed(&first, screen)?,
                screen_placed(&second, other_screen)?
            ),
            0.0,
            epsilon = 1e-12
        );
        Ok(())
    }

    /// The alignment wavelength of the source port is what decides where a grating sends the
    /// axis, and so where the setup behind it stands.
    #[test]
    fn the_alignment_wavelength_of_the_port_places_the_setup() -> OpmResult<()> {
        let light = nanometer!(1053.0);
        let (at_1053, screen) = a_setup_analyzed_with(grating()?, light, light, 0.0, None)?;
        let (at_1000, other_screen) =
            a_setup_analyzed_with(grating()?, nanometer!(1000.0), light, 0.0, None)?;
        let screen_at_1053 = screen_placed(&at_1053, screen)?;
        assert!(
            distance(screen_at_1053, screen_placed(&at_1000, other_screen)?) > 1e-3,
            "the screen stands at {screen_at_1053:?} either way"
        );
        Ok(())
    }

    /// The medium the setup is aligned in decides how a lens bends the axis, and so where the
    /// setup behind it stands.
    #[test]
    fn the_medium_of_the_setup_places_it() -> OpmResult<()> {
        let at = nanometer!(1053.0);
        let (in_vacuum, screen) = a_setup_analyzed_with(decentered_lens()?, at, at, 0.0, None)?;
        let (mut in_water, other_screen) =
            a_setup_analyzed_with(decentered_lens()?, at, at, 0.0, None)?;
        in_water.set_ambient_material(water()?);
        let screen_in_vacuum = screen_placed(&in_vacuum, screen)?;
        assert!(
            distance(screen_in_vacuum, screen_placed(&in_water, other_screen)?) > 1e-4,
            "the screen stands at {screen_in_vacuum:?} either way"
        );
        Ok(())
    }

    /// The medium an analysis traces its light through is that analysis's own: it does not move
    /// the setup.
    #[test]
    fn the_medium_of_an_analysis_does_not_place_the_setup() -> OpmResult<()> {
        let at = nanometer!(1053.0);
        let (plain, screen) = a_setup_analyzed_with(decentered_lens()?, at, at, 0.0, None)?;
        let (in_water, other_screen) =
            a_setup_analyzed_with(decentered_lens()?, at, at, 0.0, Some(water()?))?;
        assert_relative_eq!(
            distance(
                screen_placed(&plain, screen)?,
                screen_placed(&in_water, other_screen)?
            ),
            0.0,
            epsilon = 1e-12
        );
        Ok(())
    }

    /// The medium of the setup is written to file and read back.
    #[test]
    fn the_medium_of_the_setup_survives_the_file() -> OpmResult<()> {
        let at = nanometer!(1053.0);
        let (mut document, screen) = a_setup_analyzed_with(decentered_lens()?, at, at, 0.0, None)?;
        document.set_ambient_material(water()?);
        let read_back = OpmDocument::from_string(&document.to_opm_file_string()?)?;
        assert_relative_eq!(
            distance(
                screen_placed(&document, screen)?,
                screen_placed(&read_back, screen)?
            ),
            0.0,
            epsilon = 1e-12
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

        let mut placing = placing_by_distance(&document.run_positioning()?);
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
        let placing = placing_by_distance(&document.run_positioning()?);
        assert!(placing.contains(&(first, second)), "{placing:?}");
        Ok(())
    }

    /// The axis ends collected by the run, nested groups included, are handed out as the optical
    /// axis and taken out of the copy, so a trace on the copy starts without them.
    #[test]
    fn the_optical_axis_is_taken_out_of_the_positioned_copy() -> OpmResult<()> {
        let (document, _, _) = source_into_a_group()?;
        let run = document.run_positioning()?;
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
        let run = document.run_positioning()?;
        assert!(run.positioned_scenery().is_err(), "the placement fails");
        assert!(run.optical_axis().is_err(), "and with it the axis");

        let placing = placing_by_distance(&run);
        assert!(placing.contains(&(source, ahead)), "{placing:?}");
        Ok(())
    }
}
