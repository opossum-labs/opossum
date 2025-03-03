use std::sync::{Arc, Mutex};

use opossum::{
    analyzers::{
        ghostfocus::GhostFocusAnalyzer, raytrace::RayTracingAnalyzer, Analyzer, GhostFocusConfig,
        RayTraceConfig,
    },
    degree, joule, millimeter, nanometer,
    nodes::{
        round_collimated_ray_source, BeamSplitter, CylindricLens, Dummy, EnergyMeter,
        FluenceDetector, IdealFilter, Lens, NodeGroup, ParabolicMirror, ParaxialSurface,
        RayPropagationVisualizer, ReflectiveGrating, Spectrometer, SpotDiagram, ThinMirror,
        WaveFront, Wedge,
    },
    optic_node::OpticNode,
    SceneryResources,
};

fn main() {
    let mut scenery = NodeGroup::default();
    let src = round_collimated_ray_source(millimeter!(10.0), joule!(1.0), 1).unwrap();
    let i_0 = scenery.add_node(&src).unwrap();
    let i_1 = scenery.add_node(&BeamSplitter::default()).unwrap();
    // let i_2 = scenery.add_node(&CylindricLens::default()).unwrap();
    // let i_3 = scenery.add_node(&FluenceDetector::default()).unwrap();
    // let i_4 = scenery.add_node(&Lens::default()).unwrap();
    // let i_5 = scenery.add_node(&Wedge::default()).unwrap();
    // let i_6 = scenery.add_node(&Dummy::default()).unwrap();
    // let i_7 = scenery.add_node(&EnergyMeter::default()).unwrap();
    // let i_8 = scenery.add_node(&IdealFilter::default()).unwrap();
    // let i_9 = scenery
    //     .add_node(&ParaxialSurface::new("paraxial", millimeter!(1000.0)).unwrap())
    //     .unwrap();
    // let i_10 = scenery
    //     .add_node(&RayPropagationVisualizer::default())
    //     .unwrap();
    // let i_11 = scenery.add_node(&Spectrometer::default()).unwrap();
    // let i_12 = scenery.add_node(&SpotDiagram::default()).unwrap();
    // let i_13 = scenery.add_node(&WaveFront::default()).unwrap();
    // let i_14 = scenery.add_node(&ParabolicMirror::default()).unwrap();
    // let i_15 = scenery
    //     .add_node(
    //         &ReflectiveGrating::default()
    //             .with_rot_from_littrow(nanometer!(1000.0), degree!(0.0))
    //             .unwrap(),
    //     )
    //     .unwrap();
    // let i_16 = scenery.add_node(&ThinMirror::default()).unwrap();

    scenery
        .connect_nodes(&i_0, "output_1", &i_1, "input_1", millimeter!(5.0))
        .unwrap();
    // scenery
    //     .connect_nodes(&i_1, "out1_trans1_refl2", &i_2, "input_1", millimeter!(5.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_2, "output_1", &i_3, "input_1", millimeter!(5.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_3, "output_1", &i_4, "input_1", millimeter!(5.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_4, "output_1", &i_5, "input_1", millimeter!(5.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_5, "output_1", &i_6, "input_1", millimeter!(5.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_6, "output_1", &i_7, "input_1", millimeter!(5.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_7, "output_1", &i_8, "input_1", millimeter!(5.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_8, "output_1", &i_9, "input_1", millimeter!(5.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_9, "output_1", &i_10, "input_1", millimeter!(5.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_10, "output_1", &i_11, "input_1", millimeter!(5.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_11, "output_1", &i_12, "input_1", millimeter!(5.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_12, "output_1", &i_13, "input_1", millimeter!(5.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_13, "output_1", &i_14, "input_1", millimeter!(5.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_14, "output_1", &i_15, "input_1", millimeter!(50.0))
    //     .unwrap();
    // scenery
    //     .connect_nodes(&i_15, "output_1", &i_16, "input_1", millimeter!(50.0))
    //     .unwrap();

    scenery.set_global_conf(Some(Arc::new(Mutex::new(SceneryResources::default()))));
    // Perform ray tracing analysis
    testing_logger::setup();
    let analyzer = RayTracingAnalyzer::new(RayTraceConfig::default());
    analyzer.analyze(&mut scenery).unwrap();
    // scenery.reset_data();
    // // Perform ghost focus analysis
    // let analyzer = GhostFocusAnalyzer::new(GhostFocusConfig::default());
    // analyzer.analyze(&mut scenery).unwrap();
}
