//! Integration tests guaranteeing `.opm` file format stability, determinism,
//! version gating, and backward compatibility across releases.

use opossum_core::{
    analyzers::{AnalyzerType, GhostFocusConfig, RayTraceConfig},
    coatings::SplittingConfigBuilder,
    core_optics::node_attr::NodePositioning,
    degree,
    gain::{ConstGain, GainModel},
    joule,
    material::Material,
    millimeter, nanometer,
    nodes::{
        BeamSplitter, BoredMirror, CylindricLens, Dummy, EnergyMeter, FluenceDetector, IdealFilter,
        Lens, NodeGroup, NodeReference, ParabolicMirror, ParaxialSurface, RayPropagationVisualizer,
        ReflectiveGrating, SourcePort, Spectrometer, SpotDiagram, ThinMirror, WaveFront, Wedge,
        round_collimated_ray_builder,
    },
    opm_document::{
        CURRENT_OPM_FILE_VERSION, OpmDocument, OptimizationConfig,
        optimization::{
            BoxBounds, FreeVariable, MetricOperand, ObservableAddress, OperandMode,
            OptimizationAlgorithm, ParameterAddress::SurfaceCurvature,
        },
    },
    utils::geom_transformation::Isometry,
};
use std::path::Path;

/// Builds a rich reference document exercising all node types, nested groups,
/// port mappings, node references, embedded materials, multiple analyzers,
/// pump scenarios, and optimization configurations.
fn build_comprehensive_v1_document() -> opossum_core::error::OpmResult<OpmDocument> {
    let mut scenery = NodeGroup::new("Root Scenery");

    let src = scenery.add_node(SourcePort::default())?;
    scenery.with_node_attr_mut(src, |attr| {
        attr.set_positioning(NodePositioning::Absolute(Isometry::identity()));
    })?;

    // Nested group (telescope) with multiple mapped ports to test deterministic PortMap order
    let mut telescope = NodeGroup::new("Telescope");
    let l1 = telescope.add_node(Lens::new(
        "Input Lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        Material::n_bk7(),
    )?)?;
    let l2 = telescope.add_node(Lens::new(
        "Exit Lens",
        millimeter!(200.0),
        millimeter!(-200.0),
        millimeter!(15.0),
        Material::n_bk7(),
    )?)?;
    telescope.connect_nodes(l1, "output_1", l2, "input_1", millimeter!(250.0))?;
    telescope.map_input_port(l1, "input_1", "telescope_in")?;
    telescope.map_output_port(l2, "output_1", "telescope_out")?;
    let tele_id = scenery.add_node(telescope)?;

    // Additional optical components & detectors
    let bs = scenery.add_node(BeamSplitter::new(
        "Splitter",
        &SplittingConfigBuilder::FixedRatio(0.5),
    )?)?;
    let cyl = scenery.add_node(CylindricLens::default())?;
    let wedge = scenery.add_node(Wedge::default())?;
    let mirror = scenery.add_node(ThinMirror::default())?;
    let para = scenery.add_node(ParabolicMirror::default())?;
    let grating = scenery.add_node(
        ReflectiveGrating::default().with_rot_from_littrow(nanometer!(1053.0), degree!(0.0))?,
    )?;
    let bored = scenery.add_node(BoredMirror::default())?;
    let paraxial = scenery.add_node(ParaxialSurface::new("Paraxial", millimeter!(500.0))?)?;
    let filter = scenery.add_node(IdealFilter::default())?;
    let dummy = scenery.add_node(Dummy::default())?;
    let dummy_ref_target = scenery.node(dummy)?;
    let node_ref = scenery.add_node(NodeReference::from_node(&dummy_ref_target)?)?;

    let spot = scenery.add_node(SpotDiagram::default())?;
    let wf = scenery.add_node(WaveFront::default())?;
    let fluence = scenery.add_node(FluenceDetector::default())?;
    let energy = scenery.add_node(EnergyMeter::default())?;
    let spectro = scenery.add_node(Spectrometer::default())?;
    let vis = scenery.add_node(RayPropagationVisualizer::default())?;

    // Connect main optical chain
    scenery.connect_nodes(src, "output_1", tele_id, "telescope_in", millimeter!(50.0))?;
    scenery.connect_nodes(tele_id, "telescope_out", bs, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(bs, "out1_trans1_refl2", cyl, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(cyl, "output_1", wedge, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(wedge, "output_1", mirror, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(mirror, "output_1", para, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(para, "output_1", grating, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(grating, "output_1", paraxial, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(paraxial, "output_1", filter, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(filter, "output_1", dummy, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(dummy, "output_1", node_ref, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(node_ref, "output_1", wf, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(wf, "output_1", spot, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(spot, "output_1", fluence, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(fluence, "output_1", energy, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(energy, "output_1", spectro, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(spectro, "output_1", vis, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(bs, "out2_trans2_refl1", bored, "mirror", millimeter!(30.0))?;

    let mut doc = OpmDocument::new(scenery);

    // Analyzers
    let ray_builder = round_collimated_ray_builder(millimeter!(5.0), joule!(0.1), 3)?;
    let mut rt_cfg = RayTraceConfig::default();
    rt_cfg.map_source(src, ray_builder.clone());
    let rt_id = doc.add_analyzer(AnalyzerType::RayTrace(rt_cfg));

    let mut gf_cfg = GhostFocusConfig::default();
    gf_cfg.map_source(src, ray_builder);
    doc.add_analyzer(AnalyzerType::GhostFocus(gf_cfg));

    // Amplifier nodes & Pump scenarios (multiple entries to verify deterministic sorting)
    doc.set_is_amplifier_node(wedge, true);
    doc.set_is_amplifier_node(cyl, true);
    let scen_id = doc.add_pump_scenario("Main Pump");
    doc.pump_scenario_mut(scen_id)
        .unwrap()
        .set_gain_model(wedge, GainModel::Const(ConstGain::new(1.5)?));

    // Optimization config
    let opt_cfg = OptimizationConfig::new()
        .with_variable(
            FreeVariable::new(SurfaceCurvature {
                node_id: l1,
                property_name: "front radius of curvature".into(),
            })
            .with_bounds(BoxBounds::new(-40.0, 40.0)?),
        )
        .with_operand(MetricOperand::new(
            ObservableAddress {
                analyzer_id: rt_id,
                node_id: spot,
                metric_name: "rms beam radius".into(),
            },
            0.0,
            OperandMode::Equal,
            1.0,
            1e-6,
        ))
        .with_algorithm(OptimizationAlgorithm::adaptive_simulated_annealing());
    doc.set_optimization_config(Some(opt_cfg));

    Ok(doc)
}

#[test]
fn serialization_is_strictly_deterministic_and_idempotent() -> opossum_core::error::OpmResult<()> {
    let doc = build_comprehensive_v1_document()?;

    let ron_first = doc.to_opm_file_string()?;
    let ron_second = doc.to_opm_file_string()?;
    assert_eq!(
        ron_first, ron_second,
        "Serializing the exact same document twice must yield byte-identical RON"
    );

    let reloaded = OpmDocument::from_string(&ron_first)?;
    let ron_third = reloaded.to_opm_file_string()?;
    assert_eq!(
        ron_first, ron_third,
        "Reloading and re-saving an .opm file must be completely idempotent"
    );
    assert_eq!(reloaded.embedded_materials().len(), 2);
    assert!(!ron_first.contains("18446744073709551615"));

    Ok(())
}

#[test]
fn rejects_unsupported_future_and_legacy_versions() {
    let future_version = CURRENT_OPM_FILE_VERSION + 1;
    let future_ron = format!(
        r#"#![enable(unwrap_variant_newtypes)]
        (
            opm_file_version: {future_version},
            scenery: (
                node_type: "group",
                name: "future",
                uuid: "84d4007c-514e-44ca-8b63-bfa86bee265f",
                graph: (nodes: [], edges: []),
            ),
        )"#
    );
    let err = OpmDocument::from_string(&future_ron).unwrap_err();
    assert!(
        err.to_string().contains("Unsupported OPM file version"),
        "Unexpected error message for future version: {err}"
    );

    if CURRENT_OPM_FILE_VERSION > 0 {
        let legacy_version = CURRENT_OPM_FILE_VERSION - 1;
        let legacy_ron = format!(
            r#"#![enable(unwrap_variant_newtypes)]
            (
                opm_file_version: {legacy_version},
                scenery: (
                    node_type: "group",
                    name: "legacy",
                    uuid: "84d4007c-514e-44ca-8b63-bfa86bee265f",
                    graph: (nodes: [], edges: []),
                ),
            )"#
        );
        let err = OpmDocument::from_string(&legacy_ron).unwrap_err();
        assert!(
            err.to_string().contains("no longer supported"),
            "Unexpected error message for legacy version: {err}"
        );
    }
}

#[test]
fn prunes_dangling_references_when_unknown_nodes_are_skipped() -> opossum_core::error::OpmResult<()>
{
    let ron_with_unknown_amplifier = format!(
        r#"#![enable(unwrap_variant_newtypes)]
        (
            opm_file_version: {CURRENT_OPM_FILE_VERSION},
            scenery: (
                node_type: "group",
                name: "root",
                uuid: "84d4007c-514e-44ca-8b63-bfa86bee265f",
                graph: (
                    nodes: [
                        (
                            node_type: "future_quantum_amplifier",
                            name: "amp_1",
                            uuid: "11111111-1111-1111-1111-111111111111",
                        ),
                    ],
                    edges: [],
                ),
            ),
            amplifier_nodes: [
                "11111111-1111-1111-1111-111111111111",
            ],
        )"#
    );

    let doc = OpmDocument::from_string(&ron_with_unknown_amplifier)?;
    assert!(
        doc.amplifier_nodes().is_empty(),
        "Dangling amplifier UUIDs must be pruned when the referenced node fails to load"
    );
    Ok(())
}

/// Frozen Golden Fixture Test:
/// Once `files_for_testing/opm/v1_golden_reference.opm` is generated and checked in,
/// this test guarantees that future changes to `opossum_core` never silently break
/// loading of existing v1 files.
#[test]
fn golden_v1_fixture_loads_without_errors() -> opossum_core::error::OpmResult<()> {
    let fixture_path = Path::new("./files_for_testing/opm/v1_golden_reference.opm");
    if !fixture_path.exists() {
        // Generate initial v1 golden fixture if not yet checked in
        let doc = build_comprehensive_v1_document()?;
        if let Some(parent) = fixture_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        doc.save_to_file(fixture_path)?;
    }

    let loaded = OpmDocument::from_file(fixture_path)?;
    assert_eq!(loaded.opm_file_version(), 0);
    assert!(!loaded.scenery().nodes().is_empty());
    Ok(())
}
