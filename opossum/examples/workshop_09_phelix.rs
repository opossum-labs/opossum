use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    energy_distributions::UniformDist,
    error::OpmResult,
    joule,
    lightdata::{light_data_builder::LightDataBuilder, ray_data_builder::RayDataBuilder},
    millimeter, nanometer,
    nodes::{
        NodeGroup, NodeReference, ParaxialSurface, RayPropagationVisualizer, Source, ThinMirror,
        Wedge,
    },
    optic_node::{Alignable, OpticNode},
    position_distributions::Grid,
    refractive_index::RefrIndexConst,
    spectral_distribution::LaserLines,
    utils::geom_transformation::Isometry,
    OpmDocument,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("PHELIX MainAmp");

    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated {
        pos_dist: Grid::new((Length::zero(), millimeter!(60.0)), (1, 5))?.into(),
        energy_dist: UniformDist::new(joule!(1.0))?.into(),
        spect_dist: LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
    });
    let mut src = Source::new("incoming rays", light_data_builder);
    src.set_isometry(Isometry::identity())?;
    let i_src = scenery.add_node(src)?;

    let lens1 = ParaxialSurface::new("Input lens", millimeter!(1400.0))?;
    let i_l1 = scenery.add_node(lens1)?;
    let i_m1 = scenery.add_node(ThinMirror::new("mirror 1").with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    let i_m2 = scenery.add_node(ThinMirror::new("mirror 2").with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    let lens2 = ParaxialSurface::new("Input lens", millimeter!(7000.0))?;
    let i_l2 = scenery.add_node(lens2)?;

    let i_mm3 = scenery.add_node(ThinMirror::new("MM3").with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    let i_mm2 = scenery.add_node(ThinMirror::new("MM2").with_tilt(degree!(45.0, 0.0, 0.0))?)?;

    let mut amps = NodeGroup::new("Amps");

    let i_amp1 = amps.add_node(amp("Amp 1")?)?;
    let i_amp2 = amps.add_node(amp("Amp 2")?)?;
    let i_amp3 = amps.add_node(amp("Amp 3")?)?;
    let i_amp4 = amps.add_node(amp("Amp 4")?)?;
    let i_amp5 = amps.add_node(amp("Amp 5")?)?;

    amps.connect_nodes(i_amp1, "output", i_amp2, "input", millimeter!(800.0))?;
    amps.connect_nodes(i_amp2, "output", i_amp3, "input", millimeter!(800.0))?;
    amps.connect_nodes(i_amp3, "output", i_amp4, "input", millimeter!(800.0))?;
    amps.connect_nodes(i_amp4, "output", i_amp5, "input", millimeter!(800.0))?;

    amps.map_input_port(i_amp1, "input", "input")?;
    amps.map_output_port(i_amp5, "output", "output")?;

    amps.set_property("expand view", true.into())?;

    let mut main_amp = NodeGroup::new("Double-Pass Amps");

    let i_amps = main_amp.add_node(amps)?;
    let i_mm1 = main_amp.add_node(ThinMirror::new("MM1").with_tilt(degree!(-0.5, 0.0, 0.0))?)?;
    let mut r_amps = NodeReference::from_node(&main_amp.node(i_amps)?);
    r_amps.set_inverted(true)?;
    let i_r_amps = main_amp.add_node(r_amps)?;

    main_amp.connect_nodes(i_amps, "output", i_mm1, "input_1", millimeter!(800.0))?;
    main_amp.connect_nodes(i_mm1, "output_1", i_r_amps, "output", millimeter!(0.0))?;
    main_amp.map_input_port(i_amps, "input", "input")?;
    main_amp.map_output_port(i_r_amps, "input", "output")?;

    main_amp.set_property("expand view", true.into())?;
    let i_main_amp = scenery.add_node(main_amp)?;

    let mut r_mm2 = NodeReference::from_node(&scenery.node(i_mm2)?);
    r_mm2.set_inverted(true)?;
    let i_r_mm2 = scenery.add_node(r_mm2)?;

    let mut r_mm3 = NodeReference::from_node(&scenery.node(i_mm3)?);
    r_mm3.set_inverted(true)?;
    let i_r_mm3 = scenery.add_node(r_mm3)?;

    let mut r_l2 = NodeReference::from_node(&scenery.node(i_l2)?);
    r_l2.set_inverted(true)?;
    let i_r_l2 = scenery.add_node(r_l2)?;

    let lens3 = ParaxialSurface::new("Exit lens", millimeter!(7000.0))?;
    let i_l3 = scenery.add_node(lens3)?;

    let i_mm4 = scenery.add_node(ThinMirror::new("MM4").with_tilt(degree!(45.0, 0.0, 0.0))?)?;

    let mut ray_prop_vis = RayPropagationVisualizer::new("propagation", None)?;
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    let i_sd3 = scenery.add_node(ray_prop_vis)?;

    scenery.connect_nodes(i_src, "output_1", i_l1, "input_1", millimeter!(1500.0))?;
    scenery.connect_nodes(i_l1, "output_1", i_m1, "input_1", millimeter!(600.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(600.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_l2, "input_1", millimeter!(7200.0))?;
    scenery.connect_nodes(i_l2, "output_1", i_mm3, "input_1", millimeter!(500.0))?;
    scenery.connect_nodes(i_mm3, "output_1", i_mm2, "input_1", millimeter!(1500.0))?;

    // disks forward
    scenery.connect_nodes(i_mm2, "output_1", i_main_amp, "input", millimeter!(1000.0))?;
    scenery.connect_nodes(i_main_amp, "output", i_r_mm2, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_r_mm2, "input_1", i_r_mm3, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_r_mm3, "input_1", i_r_l2, "output_1", millimeter!(0.0))?;

    scenery.connect_nodes(i_r_l2, "input_1", i_l3, "input_1", millimeter!(14000.0))?;
    scenery.connect_nodes(i_l3, "output_1", i_mm4, "input_1", millimeter!(600.0))?;

    scenery.connect_nodes(i_mm4, "output_1", i_sd3, "input_1", millimeter!(2000.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/workshop_09_phelix.opm"))
}

fn amp(name: &str) -> OpmResult<NodeGroup> {
    let fused_silica = RefrIndexConst::new(1.5)?;
    let mut amp = NodeGroup::new(name);
    let disk_a = Wedge::new("disk A", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(-56.0, 0.0, 0.0))?;
    let i_a1 = amp.add_node(disk_a)?;

    let disk_a = Wedge::new("disk B", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(56.0, 0.0, 0.0))?;
    let i_a2 = amp.add_node(disk_a)?;

    amp.connect_nodes(i_a1, "output_1", i_a2, "input_1", millimeter!(900.0))?;
    amp.map_input_port(i_a1, "input_1", "input")?;
    amp.map_output_port(i_a2, "output_1", "output")?;
    amp.set_property("expand view", true.into())?;
    Ok(amp)
}
