use super::*;
use crate::{
    analyzers::{
        AnalyzerType, GhostFocusConfig, RayTraceConfig, energy::EnergyConfig,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{Alignable, node_attr::HasNodeAttr},
    degree,
    gain::{ConstGain, GainModel},
    joule,
    light::{
        LightResult,
        lightdata::{
            energy_data_builder::{EnergyDataBuilder, EnergyLaserLines},
            ray_data_builder::RayDataBuilder,
        },
    },
    millimeter, nanometer,
    nodes::{
        BeamSplitter, Dummy, Lens, NodeReference, ReflectiveGrating, SourcePort, ThinMirror,
        round_collimated_ray_builder,
    },
};
use approx::assert_relative_eq;
use uom::si::f64::Length;

#[test]
fn new() {
    let mut scenery = NodeGroup::default();
    scenery.node_attr_mut().set_name("MyTest");
    let document = OpmDocument::new(scenery);
    assert_eq!(document.scenery.node_attr().name(), "MyTest");
    assert!(document.analyzers.is_empty());
}

#[test]
fn default() {
    let document = OpmDocument::default();
    assert_eq!(document.opm_file_version, super::CURRENT_OPM_FILE_VERSION);
    assert!(document.analyzers.is_empty());
    assert!(document.pump_scenarios.is_empty());
}

#[test]
fn pump_scenario_crud() {
    let mut document = OpmDocument::default();
    let id = document.add_pump_scenario("full power");
    assert_eq!(document.pump_scenarios().len(), 1);
    assert_eq!(
        document.pump_scenario(id).map(PumpScenario::name),
        Some("full power")
    );
    assert!(document.pump_scenario(Uuid::new_v4()).is_none());

    document
        .pump_scenario_mut(id)
        .expect("the scenario just added must be there")
        .set_name("half power");
    assert_eq!(
        document.pump_scenario(id).map(PumpScenario::name),
        Some("half power")
    );

    let removed = document.remove_pump_scenario(id);
    assert_eq!(removed.as_ref().map(PumpScenario::name), Some("half power"));
    assert!(document.pump_scenarios().is_empty());
    assert!(document.remove_pump_scenario(id).is_none());

    // Restoring a scenario keeps its identity
    document.insert_pump_scenario(id, removed.expect("the scenario was removed above"));
    assert_eq!(
        document.pump_scenario(id).map(PumpScenario::name),
        Some("half power")
    );
}

#[test]
fn pruning_follows_deleted_nodes_in_every_scenario() -> OpmResult<()> {
    let mut document = OpmDocument::default();
    let lens_id = document.scenery_mut().add_node(Lens::default())?;
    let deleted_id = document.scenery_mut().add_node(Lens::default())?;
    let gain = GainModel::Const(ConstGain::new(2.0)?);
    for name in ["full power", "half power"] {
        let scenario_id = document.add_pump_scenario(name);
        let scenario = document
            .pump_scenario_mut(scenario_id)
            .expect("the scenario just added must be there");
        scenario.set_gain_model(lens_id, gain);
        scenario.set_gain_model(deleted_id, gain);
    }
    document.scenery_mut().delete_node(deleted_id)?;
    document.prune_pump_scenarios();

    for scenario in document.pump_scenarios().values() {
        assert_eq!(scenario.gain_model(lens_id), gain);
        assert_eq!(scenario.gain_model(deleted_id), GainModel::None);
    }
    Ok(())
}

#[test]
fn amplifier_node_candidacy_roundtrip() -> OpmResult<()> {
    let mut document = OpmDocument::default();
    let lens_id = document.scenery_mut().add_node(Lens::default())?;
    assert!(!document.is_amplifier_node(lens_id));
    assert!(document.amplifier_nodes().is_empty());

    document.set_is_amplifier_node(lens_id, true);
    assert!(document.is_amplifier_node(lens_id));
    assert_eq!(document.amplifier_nodes(), &HashSet::from([lens_id]));

    document.set_is_amplifier_node(lens_id, false);
    assert!(!document.is_amplifier_node(lens_id));
    assert!(document.amplifier_nodes().is_empty());
    Ok(())
}

#[test]
fn unmarking_a_candidate_wipes_its_gain_model_in_every_scenario() -> OpmResult<()> {
    let mut document = OpmDocument::default();
    let lens_id = document.scenery_mut().add_node(Lens::default())?;
    document.set_is_amplifier_node(lens_id, true);
    let gain = GainModel::Const(ConstGain::new(2.0)?);
    let full_power = document.add_pump_scenario("full power");
    let half_power = document.add_pump_scenario("half power");
    document
        .pump_scenario_mut(full_power)
        .expect("the scenario just added must be there")
        .set_gain_model(lens_id, gain);
    document
        .pump_scenario_mut(half_power)
        .expect("the scenario just added must be there")
        .set_gain_model(lens_id, gain);

    document.set_is_amplifier_node(lens_id, false);

    for scenario_id in [full_power, half_power] {
        assert_eq!(
            document
                .pump_scenario(scenario_id)
                .map(|scenario| scenario.gain_model(lens_id)),
            Some(GainModel::None)
        );
    }
    Ok(())
}

#[test]
fn set_amplifier_nodes_replaces_the_whole_set_without_touching_scenarios() -> OpmResult<()> {
    let mut document = OpmDocument::default();
    let lens_id = document.scenery_mut().add_node(Lens::default())?;
    let scenario_id = document.add_pump_scenario("full power");
    let gain = GainModel::Const(ConstGain::new(2.0)?);
    document
        .pump_scenario_mut(scenario_id)
        .expect("the scenario just added must be there")
        .set_gain_model(lens_id, gain);

    document.set_amplifier_nodes(HashSet::new());
    assert!(document.amplifier_nodes().is_empty());
    assert_eq!(
        document
            .pump_scenario(scenario_id)
            .map(|scenario| scenario.gain_model(lens_id)),
        Some(gain),
        "a whole-set replace must not wipe any scenario's gain model"
    );

    document.set_amplifier_nodes(HashSet::from([lens_id]));
    assert_eq!(document.amplifier_nodes(), &HashSet::from([lens_id]));
    Ok(())
}

#[test]
fn prune_amplifier_nodes_drops_deleted_node_entries() -> OpmResult<()> {
    let mut document = OpmDocument::default();
    let lens_id = document.scenery_mut().add_node(Lens::default())?;
    let deleted_id = document.scenery_mut().add_node(Lens::default())?;
    document.set_is_amplifier_node(lens_id, true);
    document.set_is_amplifier_node(deleted_id, true);
    document.scenery_mut().delete_node(deleted_id)?;

    document.prune_amplifier_nodes();

    assert!(document.is_amplifier_node(lens_id));
    assert!(!document.is_amplifier_node(deleted_id));
    Ok(())
}

#[test]
fn add_analyzer() {
    let mut document = OpmDocument::default();
    assert!(document.analyzers.is_empty());
    document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
    assert_eq!(document.analyzers.len(), 1);
}

#[test]
fn add_analyzer_with_position() {
    let mut document = OpmDocument::default();
    let uuid =
        document.add_analyzer_with_position(AnalyzerType::Energy(EnergyConfig::default()), None);
    assert!(!uuid.is_nil());
}

#[test]
fn analyzers() {
    let mut document = OpmDocument::default();
    document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
    document.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    assert_eq!(document.analyzers().len(), 2);
}

#[test]
fn analyzer() {
    let mut document = OpmDocument::default();
    let uuid1 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
    let uuid2 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));

    assert!(document.analyzer(uuid1).is_ok());
    assert!(document.analyzer(uuid2).is_ok());
    assert!(document.analyzer(Uuid::nil()).is_err());
}

#[test]
fn analyzer_mut() {
    let mut document = OpmDocument::default();
    let uuid1 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
    let uuid2 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));

    assert!(document.analyzer_mut(uuid1).is_some());
    assert!(document.analyzer_mut(uuid2).is_some());
    assert!(document.analyzer_mut(Uuid::nil()).is_none());
}

#[test]
fn remove_analyzer() {
    let mut document = OpmDocument::default();
    let uuid1 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
    let uuid2 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));

    assert!(document.remove_analyzer(uuid1).is_ok());
    assert_eq!(document.analyzers.len(), 1);
    assert!(document.remove_analyzer(Uuid::nil()).is_err());
    assert!(document.remove_analyzer(uuid2).is_ok());
    assert!(document.analyzers.is_empty());
}

/// A source port, a component 100 mm along and another 50 mm behind that.
///
/// The components are [`Dummy`]s, which have no thickness of their own, so the placements the
/// positioning run arrives at are exactly the distances the connections state.
fn lined_up_model() -> OpmResult<(OpmDocument, Uuid, Uuid, Uuid)> {
    let mut scenery = NodeGroup::default();
    let source = scenery.add_node(SourcePort::default())?;
    let first = scenery.add_node(Dummy::default())?;
    let second = scenery.add_node(Dummy::default())?;
    scenery.connect_nodes(source, "output_1", first, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(first, "output_1", second, "input_1", millimeter!(50.0))?;
    Ok((OpmDocument::new(scenery), source, first, second))
}

/// Where along the optical axis a node of the given document sits, in meter.
fn placed_at(document: &OpmDocument, node_id: Uuid) -> OpmResult<f64> {
    let (node_ref, _) = document.scenery().node_recursive(node_id)?;
    let placement = node_ref.node_attr().effective_position();
    Ok(placement
        .expect("this node was expected to be placed")
        .translation()
        .z
        .value)
}

fn is_placed(document: &OpmDocument, node_id: Uuid) -> OpmResult<bool> {
    let (node_ref, _) = document.scenery().node_recursive(node_id)?;
    Ok(node_ref.node_attr().effective_position().is_some())
}

/// A placement is saved with the model and a later run leaves an already placed node alone, so
/// drawing a setup must not reach back into the document it was asked about.
#[test]
fn a_positioned_copy_leaves_the_document_it_came_from_alone() -> OpmResult<()> {
    let (mut document, source, first, second) = lined_up_model()?;
    let mut config = RayTraceConfig::default();
    config.map_source(source, RayDataBuilder::default());
    document.add_analyzer(AnalyzerType::RayTrace(config));
    let before = document.to_opm_file_string()?;

    let placed = document.positioned_copy(None)?;

    assert_eq!(document.to_opm_file_string()?, before);
    assert!(!is_placed(&document, first)?);
    assert!(!is_placed(&document, second)?);
    // The distances of the connections, measured from the source port in the origin.
    assert_relative_eq!(placed_at(&placed, first)?, 0.1, epsilon = 1e-12);
    assert_relative_eq!(placed_at(&placed, second)?, 0.15, epsilon = 1e-12);
    Ok(())
}

/// Without an analyzer there is no source data, but the model still says where its own light
/// comes in — default rays from there are enough to lay the setup out.
#[test]
fn a_model_without_an_analyzer_is_placed_from_its_own_source_ports() -> OpmResult<()> {
    let (document, _, first, second) = lined_up_model()?;
    let placed = document.positioned_copy(None)?;
    assert_relative_eq!(placed_at(&placed, first)?, 0.1, epsilon = 1e-12);
    assert_relative_eq!(placed_at(&placed, second)?, 0.15, epsilon = 1e-12);
    assert_eq!(placed.scenery().find_source_ports()?.len(), 1);
    Ok(())
}

/// A model that was never analyzed may mark no source at all. Rather than leaving it unplaced,
/// one is put in front of it — in the copy only.
#[test]
fn a_model_without_a_source_is_placed_from_its_first_component() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let first = scenery.add_node(Dummy::default())?;
    let second = scenery.add_node(Dummy::default())?;
    scenery.connect_nodes(first, "output_1", second, "input_1", millimeter!(50.0))?;
    let document = OpmDocument::new(scenery);

    let placed = document.positioned_copy(None)?;

    assert_relative_eq!(placed_at(&placed, first)?, 0.0, epsilon = 1e-12);
    assert_relative_eq!(placed_at(&placed, second)?, 0.05, epsilon = 1e-12);
    assert_eq!(placed.scenery().find_source_ports()?.len(), 1);
    assert!(document.scenery().find_source_ports()?.is_empty());
    Ok(())
}

/// Two analyses can place the same model differently, so a drawing follows the one named, or else
/// the first. Named, a ghost focus analysis that defines no light for the source stops the run;
/// unnamed, the ray trace added before it places the model.
#[test]
fn a_model_analyzed_several_ways_is_drawn_after_its_first_analysis_unless_one_is_named()
-> OpmResult<()> {
    let (mut document, source, first, _) = lined_up_model()?;
    let mut config = RayTraceConfig::default();
    config.map_source(source, RayDataBuilder::default());
    document.add_analyzer(AnalyzerType::RayTrace(config));
    let ghost_focus = document.add_analyzer(AnalyzerType::GhostFocus(GhostFocusConfig::default()));

    let placed = document.positioned_copy(None)?;
    assert_relative_eq!(placed_at(&placed, first)?, 0.1, epsilon = 1e-12);

    assert!(document.positioned_copy(Some(ghost_focus)).is_err());
    Ok(())
}

/// A drawing cannot follow an analyzer the model does not have.
#[test]
fn a_model_is_not_drawn_after_an_analyzer_it_does_not_have() -> OpmResult<()> {
    let (document, ..) = lined_up_model()?;
    assert!(document.positioned_copy(Some(Uuid::new_v4())).is_err());
    Ok(())
}

/// A model with nothing in it places nothing. That is an empty answer, not a failure — a view
/// of an empty document should come out empty rather than refuse.
#[test]
fn an_empty_model_is_placed_without_complaint() -> OpmResult<()> {
    let placed = OpmDocument::default().positioned_copy(None)?;
    assert_eq!(placed.scenery().nr_of_nodes(), 0);
    Ok(())
}

/// The placement a document that went through its file form arrives at: what a placement copy
/// started from before it was a plain clone, and so the oracle the clone has to agree with.
fn placed_through_file(document: &OpmDocument) -> OpmResult<OpmDocument> {
    OpmDocument::from_string(&document.to_opm_file_string()?)?.positioned_copy(None)
}

/// Where a node sits along the optical axis, or `None` if it was not placed.
fn z_of(document: &OpmDocument, node_id: Uuid) -> OpmResult<Option<f64>> {
    let (node_ref, _) = document.scenery().node_recursive(node_id)?;
    Ok(node_ref
        .node_attr()
        .effective_position()
        .map(|placement| placement.translation().z.value))
}

/// A property changed on the live document, with nothing rebuilt afterwards, has to reach the
/// placement. The file form rebuilt every node's surfaces from its properties on the way in; a
/// plain clone only carries over what the live node holds.
#[test]
fn a_property_changed_after_adding_the_node_reaches_the_placement() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let source = scenery.add_node(SourcePort::default())?;
    let lens = scenery.add_node(Lens::default())?;
    let after = scenery.add_node(Dummy::default())?;
    scenery.connect_nodes(source, "output_1", lens, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(lens, "output_1", after, "input_1", millimeter!(50.0))?;
    let mut document = OpmDocument::new(scenery);
    document.add_analyzer(AnalyzerType::RayTrace(single_ray_config(source)?));
    let default_thickness = document.positioned_copy(None)?;

    document
        .scenery_mut()
        .graph_mut()
        .node_recursive_mut(lens)?
        .node_attr_mut()
        .set_property("center thickness", millimeter!(30.0).into())?;

    let placed = document.positioned_copy(None)?;
    let oracle = placed_through_file(&document)?;
    assert_eq!(z_of(&placed, after)?, z_of(&oracle, after)?);
    assert_ne!(
        z_of(&placed, after)?,
        z_of(&default_thickness, after)?,
        "the thicker lens has to push the component behind it"
    );
    Ok(())
}

/// A group, a node passed a second time through a reference, and a tilted mirror between them:
/// everything the clone has to copy besides plain nodes. The placement is the one the file form
/// gives, for the nodes inside the group as well, and the document it came from stays unplaced.
#[test]
fn a_group_and_a_reference_are_placed_as_through_the_file_form() -> OpmResult<()> {
    let mut group = NodeGroup::new("group");
    let inner_first = group.add_node(Dummy::default())?;
    let inner_second = group.add_node(Dummy::default())?;
    group.connect_nodes(
        inner_first,
        "output_1",
        inner_second,
        "input_1",
        millimeter!(20.0),
    )?;
    group.map_input_port(inner_first, "input_1", "input_1")?;
    group.map_output_port(inner_second, "output_1", "output_1")?;

    let mut scenery = NodeGroup::default();
    let source = scenery.add_node(SourcePort::default())?;
    let group = scenery.add_node(group)?;
    let fold = scenery.add_node(ThinMirror::new("fold").with_tilt(degree!(2.0, 0.0, 0.0))?)?;
    let second_pass = scenery.add_node(NodeReference::from_node(&scenery.node(group)?)?)?;
    scenery.connect_nodes(source, "output_1", group, "input_1", millimeter!(30.0))?;
    scenery.connect_nodes(group, "output_1", fold, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(fold, "output_1", second_pass, "input_1", millimeter!(50.0))?;
    let mut document = OpmDocument::new(scenery);
    document.add_analyzer(AnalyzerType::RayTrace(single_ray_config(source)?));
    let before = document.to_opm_file_string()?;

    let placed = document.positioned_copy(None)?;
    let oracle = placed_through_file(&document)?;

    for node_id in [source, group, inner_first, inner_second, fold, second_pass] {
        assert_eq!(z_of(&placed, node_id)?, z_of(&oracle, node_id)?);
    }
    assert!(is_placed(&placed, inner_second)?);
    assert!(is_placed(&placed, fold)?);
    assert_eq!(document.to_opm_file_string()?, before);
    assert!(!is_placed(&document, inner_second)?);
    Ok(())
}

/// A source port, an untilted default grating 50 mm along and, if asked for, a mirror 50 mm after
/// that.
///
/// Untilted, the grating's order -1 does not propagate at 1000 nm (|m·λ·g| = 1.74 > 1), so the
/// optical axis ends at the grating.
fn source_then_untilted_grating(mirror_after: bool) -> OpmResult<(NodeGroup, Uuid, Uuid)> {
    let mut scenery = NodeGroup::default();
    let source = scenery.add_node(SourcePort::default())?;
    let grating = scenery.add_node(ReflectiveGrating::default())?;
    scenery.connect_nodes(source, "output_1", grating, "input_1", millimeter!(50.0))?;
    if mirror_after {
        let mirror = scenery.add_node(ThinMirror::default())?;
        scenery.connect_nodes(grating, "output_1", mirror, "input_1", millimeter!(50.0))?;
    }
    Ok((scenery, source, grating))
}

/// A ray-trace configuration feeding the given source port a single ray at 1000 nm.
fn single_ray_config(source: Uuid) -> OpmResult<RayTraceConfig> {
    let mut config = RayTraceConfig::default();
    config.map_source(
        source,
        round_collimated_ray_builder(millimeter!(5.0), joule!(1.0), 0)?,
    );
    Ok(config)
}

/// An untilted grating swallows the optical axis, so the mirror after it cannot be placed. That has
/// to stop the run - but with a message that names the grating and says how to tilt it, rather than
/// the bare "empty ray bundle, cannot define up-direction" it used to be.
#[test]
fn positioning_stops_at_an_evanescent_grating_with_a_clear_error() -> OpmResult<()> {
    let (scenery, source, _) = source_then_untilted_grating(true)?;
    let mut document = OpmDocument::new(scenery);
    document.add_analyzer(AnalyzerType::RayTrace(single_ray_config(source)?));

    let Err(error) = document.positioned_copy(None) else {
        panic!("the mirror after the grating cannot be placed");
    };
    let error = error.to_string();

    assert!(error.contains("'reflective grating'"), "{error}");
    assert!(error.contains("evanescent"), "{error}");
    assert!(error.contains("Littrow angle of -60.46°"), "{error}");
    Ok(())
}

/// With nothing after it, a grating that swallows the optical axis leaves nothing unplaced, while
/// the light does reach the grating itself - so the run finishes and the grating can be drawn. Its
/// empty outgoing bundle used to fail the whole run.
#[test]
fn a_terminal_evanescent_grating_is_still_placed() -> OpmResult<()> {
    let (scenery, source, grating) = source_then_untilted_grating(false)?;
    let mut document = OpmDocument::new(scenery);
    document.add_analyzer(AnalyzerType::RayTrace(single_ray_config(source)?));

    let placed = document.positioned_copy(None)?;

    assert!(is_placed(&placed, grating)?);
    Ok(())
}

/// Any node that lets no light out ends the optical axis, not only a grating. A run that is not
/// flagged as positioning skips the grating's own check and hands its empty bundle on, which shows
/// that the graph walk itself says where the axis was lost.
#[test]
fn an_empty_outgoing_bundle_names_the_node_and_port() -> OpmResult<()> {
    let (mut scenery, source, _) = source_then_untilted_grating(true)?;

    let Err(error) = AnalysisRayTrace::calc_node_positions(
        &mut scenery,
        LightResult::default(),
        &single_ray_config(source)?,
    ) else {
        panic!("the mirror after the grating cannot be placed");
    };
    let error = error.to_string();

    assert!(error.contains("no light leaves"), "{error}");
    assert!(error.contains("'output_1'"), "{error}");
    Ok(())
}

/// With nothing after it, a node that lets no light out leaves nothing unplaced, whoever notices
/// the empty bundle - here the graph walk, as the run is not flagged as positioning.
#[test]
fn an_empty_outgoing_bundle_at_the_last_node_stops_nothing() -> OpmResult<()> {
    let (mut scenery, source, grating) = source_then_untilted_grating(false)?;

    AnalysisRayTrace::calc_node_positions(
        &mut scenery,
        LightResult::default(),
        &single_ray_config(source)?,
    )?;

    assert!(is_placed(&OpmDocument::new(scenery), grating)?);
    Ok(())
}

/// An energy analysis whose only source shines a single laser line at `wavelength`.
fn energy_config_at(source: Uuid, wavelength: Length) -> OpmResult<EnergyConfig> {
    let mut config = EnergyConfig::default();
    config.map_source(
        source,
        EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
            vec![(wavelength, joule!(1.0))],
            nanometer!(1.0),
        )?),
    );
    Ok(config)
}

/// An energy analysis places a model at the wavelength of its own light. The untilted grating
/// that swallows the optical axis at 1000 nm lets it through at 500 nm (|m·λ·g| = 0.87), so the
/// run gets past the grating to the mirror behind it.
#[test]
fn an_energy_analysis_places_the_model_at_the_wavelength_of_its_light() -> OpmResult<()> {
    let (scenery, source, grating) = source_then_untilted_grating(true)?;
    let mut document = OpmDocument::new(scenery);
    document.add_analyzer(AnalyzerType::Energy(energy_config_at(
        source,
        nanometer!(500.0),
    )?));

    let placed = document.positioned_copy(None)?;

    assert!(is_placed(&placed, grating)?);
    Ok(())
}

/// An energy analysis only knows a wavelength for each source, while one whose light is rays
/// knows how that light runs. Unless the energy analysis is named, a drawing follows the one with
/// rays, even when it comes second: here the ray trace at 1000 nm, at which the untilted grating
/// swallows the axis.
#[test]
fn an_energy_analysis_is_passed_over_for_one_whose_light_is_rays() -> OpmResult<()> {
    let (scenery, source, _) = source_then_untilted_grating(true)?;
    let mut document = OpmDocument::new(scenery);
    let energy = document.add_analyzer(AnalyzerType::Energy(energy_config_at(
        source,
        nanometer!(500.0),
    )?));
    document.add_analyzer(AnalyzerType::RayTrace(single_ray_config(source)?));

    let Err(error) = document.positioned_copy(None) else {
        panic!("the ray trace's axis ends at the grating");
    };
    assert!(error.to_string().contains("evanescent"), "{error}");
    assert!(document.positioned_copy(Some(energy)).is_ok());
    Ok(())
}

/// An energy analysis places a model, but it has no rays of its own to show.
#[test]
fn an_energy_analysis_has_no_rays_to_draw() -> OpmResult<()> {
    let (scenery, source, _) = source_then_untilted_grating(true)?;
    let mut document = OpmDocument::new(scenery);
    document.add_analyzer(AnalyzerType::Energy(energy_config_at(
        source,
        nanometer!(500.0),
    )?));

    assert!(document.traced_rays(None, 100)?.is_empty());
    Ok(())
}

/// A strand no source feeds - a block pasted next to the setup, say - is left unplaced, and the
/// rest is placed all the same. A beam splitter in it with something after it used to stop the
/// whole run, as the only component that fails without an incoming axis.
#[test]
fn a_strand_the_axis_does_not_reach_leaves_the_rest_placed() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let source = scenery.add_node(SourcePort::default())?;
    let lens = scenery.add_node(Lens::default())?;
    scenery.connect_nodes(source, "output_1", lens, "input_1", millimeter!(50.0))?;
    let loose_lens = scenery.add_node(Lens::default())?;
    let splitter = scenery.add_node(BeamSplitter::default())?;
    let after = scenery.add_node(Dummy::default())?;
    scenery.connect_nodes(
        loose_lens,
        "output_1",
        splitter,
        "input_1",
        millimeter!(50.0),
    )?;
    scenery.connect_nodes(
        splitter,
        "out1_trans1_refl2",
        after,
        "input_1",
        millimeter!(50.0),
    )?;
    let mut document = OpmDocument::new(scenery);
    document.add_analyzer(AnalyzerType::RayTrace(single_ray_config(source)?));

    let placed = document.positioned_copy(None)?;

    assert!(is_placed(&placed, lens)?);
    for unreached in [loose_lens, splitter, after] {
        assert!(!is_placed(&placed, unreached)?);
    }
    Ok(())
}
