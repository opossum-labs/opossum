//! Generates the `.opm` fixture used by the `opm_document::test::all_nodes_roundtrip_is_stable`
//! regression test: every registered node type (plus a nested group with mapped ports and a reference
//! node into it) and every analyzer type, so a load -> re-save round trip exercises the whole model
//! schema at once.
//!
//! Regenerate with `cargo run -p opossum_core --example all_nodes_roundtrip_fixture` after adding a new
//! node/analyzer type or changing the serialized schema, then re-check in the resulting
//! `files_for_testing/opm/all_nodes_roundtrip.opm`.
use opossum_core::prelude::*;
use std::path::Path;

// One long, linear chain of node construction and wiring by design (it mirrors the fixture file it
// produces one-to-one); splitting it into helpers would obscure that rather than clarify it.
#[allow(clippy::too_many_lines)]
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("All node types round-trip test");

    let src = scenery.add_node(SourcePort::default())?;
    let beam_splitter = scenery.add_node(BeamSplitter::default())?;
    let cylindric_lens = scenery.add_node(CylindricLens::default())?;
    let fluence_detector = scenery.add_node(FluenceDetector::default())?;
    let lens = scenery.add_node(Lens::default())?;
    let wedge = scenery.add_node(Wedge::default())?;
    let dummy = scenery.add_node(Dummy::default())?;

    // A nested group with a single mapped input and a single mapped output port, spliced into the main
    // chain - regression coverage for issue #1144 (a group with mapped ports used to vanish on reload).
    let mut inner = NodeGroup::new("Nested Group");
    let n_in = inner.add_node(Dummy::new("nested dummy in"))?;
    let n_out = inner.add_node(Dummy::new("nested dummy out"))?;
    inner.connect_nodes(n_in, "output_1", n_out, "input_1", millimeter!(5.0))?;
    inner.map_input_port(n_in, "input_1", "input_1")?;
    inner.map_output_port(n_out, "output_1", "output_1")?;
    // A reference into the nested group. Captured before `inner` is moved into `scenery` below - the
    // `OpticRef` shares the underlying node regardless of which parent group it ends up in.
    let n_in_ref = inner.node_recursive(n_in)?.0;
    let inner_group = scenery.add_node(inner)?;
    let _reference = scenery.add_node(NodeReference::from_node(&n_in_ref)?)?;

    let energy_meter = scenery.add_node(EnergyMeter::default())?;
    let ideal_filter = scenery.add_node(IdealFilter::default())?;
    let paraxial_surface =
        scenery.add_node(ParaxialSurface::new("paraxial", millimeter!(1000.0))?)?;
    let ray_propagation_visualizer = scenery.add_node(RayPropagationVisualizer::default())?;
    let spectrometer = scenery.add_node(Spectrometer::default())?;
    let spot_diagram = scenery.add_node(SpotDiagram::default())?;
    let wavefront = scenery.add_node(WaveFront::default())?;
    let parabolic_mirror = scenery.add_node(ParabolicMirror::default())?;
    let reflective_grating = scenery.add_node(
        ReflectiveGrating::default().with_rot_from_littrow(nanometer!(1000.0), degree!(0.0))?,
    )?;
    let thin_mirror = scenery.add_node(ThinMirror::default())?;

    scenery.connect_nodes(src, "output_1", beam_splitter, "input_1", millimeter!(5.0))?;
    scenery.connect_nodes(
        beam_splitter,
        "out1_trans1_refl2",
        cylindric_lens,
        "input_1",
        millimeter!(5.0),
    )?;
    scenery.connect_nodes(
        cylindric_lens,
        "output_1",
        fluence_detector,
        "input_1",
        millimeter!(5.0),
    )?;
    scenery.connect_nodes(
        fluence_detector,
        "output_1",
        lens,
        "input_1",
        millimeter!(5.0),
    )?;
    scenery.connect_nodes(lens, "output_1", wedge, "input_1", millimeter!(5.0))?;
    scenery.connect_nodes(wedge, "output_1", dummy, "input_1", millimeter!(5.0))?;
    scenery.connect_nodes(dummy, "output_1", inner_group, "input_1", millimeter!(5.0))?;
    scenery.connect_nodes(
        inner_group,
        "output_1",
        energy_meter,
        "input_1",
        millimeter!(5.0),
    )?;
    scenery.connect_nodes(
        energy_meter,
        "output_1",
        ideal_filter,
        "input_1",
        millimeter!(5.0),
    )?;
    scenery.connect_nodes(
        ideal_filter,
        "output_1",
        paraxial_surface,
        "input_1",
        millimeter!(5.0),
    )?;
    scenery.connect_nodes(
        paraxial_surface,
        "output_1",
        ray_propagation_visualizer,
        "input_1",
        millimeter!(5.0),
    )?;
    scenery.connect_nodes(
        ray_propagation_visualizer,
        "output_1",
        spectrometer,
        "input_1",
        millimeter!(5.0),
    )?;
    scenery.connect_nodes(
        spectrometer,
        "output_1",
        spot_diagram,
        "input_1",
        millimeter!(5.0),
    )?;
    scenery.connect_nodes(
        spot_diagram,
        "output_1",
        wavefront,
        "input_1",
        millimeter!(5.0),
    )?;
    scenery.connect_nodes(
        wavefront,
        "output_1",
        parabolic_mirror,
        "input_1",
        millimeter!(5.0),
    )?;
    scenery.connect_nodes(
        parabolic_mirror,
        "output_1",
        reflective_grating,
        "input_1",
        millimeter!(50.0),
    )?;
    scenery.connect_nodes(
        reflective_grating,
        "output_1",
        thin_mirror,
        "input_1",
        millimeter!(50.0),
    )?;

    let mut doc = OpmDocument::new(scenery);

    // One of each analyzer type, each mapped to the single source port. Every `source_map` therefore
    // has exactly one entry, which keeps its RON output stable across runs (a `HashMap` with more than
    // one entry has no guaranteed iteration order, which would make the round-trip fixture flaky).
    let mut energy_config = EnergyConfig::default();
    energy_config.map_source(src, EnergyDataBuilder::default());
    doc.add_analyzer(AnalyzerType::Energy(energy_config));

    let ray_builder = round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?;
    let mut ray_config = RayTraceConfig::default();
    ray_config.map_source(src, ray_builder.clone());
    doc.add_analyzer(AnalyzerType::RayTrace(ray_config));

    let mut ghost_config = GhostFocusConfig::default();
    ghost_config.map_source(src, ray_builder);
    doc.add_analyzer(AnalyzerType::GhostFocus(ghost_config));

    doc.save_to_file(Path::new(
        "./opossum_core/files_for_testing/opm/all_nodes_roundtrip.opm",
    ))
}
