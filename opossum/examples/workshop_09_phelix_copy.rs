use nalgebra::Vector3;
use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig}, degree, energy_distributions::UniformDist, error::OpmResult, joule, lightdata::{light_data_builder::LightDataBuilder, ray_data_builder::{PointSrc, RayDataBuilder}}, millimeter, nanometer, nodes::{NodeGroup, ParaxialSurface, RayPropagationVisualizer, Source, ThinMirror, Wedge}, optic_node::{Alignable, OpticNode}, position_distributions::{Grid, Hexapolar}, properties::Proptype, refractive_index::RefrIndexConst, spectral_distribution::LaserLines, utils::geom_transformation::Isometry, OpmDocument
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("PHELIX MainAmp");

    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::PointSrc (PointSrc::new(
        Grid::new((millimeter!(60.0), Length::zero()), (5, 1))?.into(),
        // pos_dist: Hexapolar::new(millimeter!(30.0), 3)?.into(),
        UniformDist::new(joule!(1.0))?.into(),
        LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
        millimeter!(1000.0))
    ));

    // let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated {
    //     pos_dist: Grid::new((millimeter!(60.0), Length::zero()), (5, 1))?.into(),
    //     // pos_dist: Hexapolar::new(millimeter!(30.0), 3)?.into(),
    //     energy_dist: UniformDist::new(joule!(1.0))?.into(),
    //     spect_dist: LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
    // });

    let mut src = Source::new("incoming rays DM", light_data_builder);
    src.set_isometry(Isometry::identity())?;
    let i_src = scenery.add_node(src)?;

    let i_l_pa_4_input = scenery.add_node(ParaxialSurface::new("T4 Input", millimeter!(931.0))?)?;
    let i_l_pa_4_exit = scenery.add_node(ParaxialSurface::new("T4 Exit", millimeter!(931.0))?)?;
    let i_m_pa_45 =
        scenery.add_node(ThinMirror::new("bridge_input").with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    let i_m_bridge =
        scenery.add_node(ThinMirror::new("bridge_input").with_tilt(degree!(0.0, 45.0, 0.0))?)?;
    let i_br_input =
        scenery.add_node(ParaxialSurface::new("bridge_input", millimeter!(2250.0))?)?;
    let i_br_exit = scenery.add_node(ParaxialSurface::new("bridge_exit", millimeter!(2250.0))?)?;
    let i_per_oben =
        scenery.add_node(ThinMirror::new("periscope_upper").with_tilt(degree!(0.0, 45.0, 0.0))?)?;
    let i_per_unten = scenery
        .add_node(ThinMirror::new("periscope_lower").with_tilt(degree!(-45.0, 0.0, 0.0))?)?;

    let lens1 = ParaxialSurface::new("Input lens", millimeter!(1890.0))?;
    let i_l1 = scenery.add_node(lens1)?;
    let i_m1 = scenery.add_node(ThinMirror::new("mirror 1").with_tilt(degree!(0.0, 45.0, 0.0))?)?;
    let i_m2 = scenery.add_node(ThinMirror::new("mirror 2").with_tilt(degree!(0.0, 45.0, 0.0))?)?;
    let lens2 = ParaxialSurface::new("Input lens", millimeter!(7590.0))?;
    let i_l2 = scenery.add_node(lens2)?;

    let i_mm3 = scenery.add_node(ThinMirror::new("MM3").with_tilt(degree!(0.0, 45.0, 0.0))?)?;
    let i_mm2 = scenery.add_node(ThinMirror::new("MM2").with_tilt(degree!(0.0, 45.0, 0.0))?)?;

    let mut amps = NodeGroup::new("Amps");

    let i_amp1 = amps.add_node(amp("Amp 1")?)?;
    let i_amp2 = amps.add_node(amp("Amp 2")?)?;
    let i_amp3 = amps.add_node(amp("Amp 3")?)?;
    let i_amp4 = amps.add_node(amp("Amp 4")?)?;
    let i_amp5 = amps.add_node(amp("Amp 5")?)?;

    amps.connect_nodes(i_amp1, "output", i_amp2, "input", millimeter!(880.0))?;
    amps.connect_nodes(i_amp2, "output", i_amp3, "input", millimeter!(880.0))?;
    amps.connect_nodes(i_amp3, "output", i_amp4, "input", millimeter!(880.0))?;
    amps.connect_nodes(i_amp4, "output", i_amp5, "input", millimeter!(880.0))?;

    amps.map_input_port(i_amp1, "input", "input")?;
    amps.map_output_port(i_amp5, "output", "output")?;

    let mut main_amp = NodeGroup::new("Double-Pass Amps");

    let i_amps = main_amp.add_node(amps)?;
    let i_mm1 = main_amp.add_node(ThinMirror::new("MM1"))?;

    main_amp.connect_nodes(i_amps, "output", i_mm1, "input_1", millimeter!(1200.0))?;
    main_amp.map_input_port(i_amps, "input", "input")?;
    main_amp.map_output_port(i_mm1, "output_1", "output")?;

    let i_main_amp = scenery.add_node(main_amp)?;

    let mut ray_prop_vis = RayPropagationVisualizer::new("propagation", None)?;
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    ray_prop_vis.set_property("view_direction", Proptype::Vec3(Vector3::y()))?;
    let i_sd3 = scenery.add_node(ray_prop_vis)?;

    scenery.connect_nodes(
        i_src,
        "output_1",
        i_l_pa_4_input,
        "input_1",
        millimeter!(1110.0),
    )?; // original 1310 mm
    scenery.connect_nodes(
        i_l_pa_4_input,
        "output_1",
        i_l_pa_4_exit,
        "input_1",
        millimeter!(1862.0),
    )?;
    scenery.connect_nodes(
        i_l_pa_4_exit,
        "output_1",
        i_m_pa_45,
        "input_1",
        millimeter!(210.0),
    )?;
    scenery.connect_nodes(
        i_m_pa_45,
        "output_1",
        i_m_bridge,
        "input_1",
        millimeter!(1940.0),
    )?;
    scenery.connect_nodes(
        i_m_bridge,
        "output_1",
        i_br_input,
        "input_1",
        millimeter!(540.0),
    )?;
    scenery.connect_nodes(
        i_br_input,
        "output_1",
        i_br_exit,
        "input_1",
        millimeter!(4500.0),
    )?;
    scenery.connect_nodes(
        i_br_exit,
        "output_1",
        i_per_oben,
        "input_1",
        millimeter!(1250.0),
    )?;
    scenery.connect_nodes(
        i_per_oben,
        "output_1",
        i_per_unten,
        "input_1",
        millimeter!(1660.0),
    )?;
    scenery.connect_nodes(
        i_per_unten,
        "output_1",
        i_l1,
        "input_1",
        millimeter!(1160.0),
    )?;
    scenery.connect_nodes(i_l1, "output_1", i_m1, "input_1", millimeter!(330.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(1120.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_l2, "input_1", millimeter!(8030.0))?;
    scenery.connect_nodes(i_l2, "output_1", i_mm3, "input_1", millimeter!(800.0))?;
    scenery.connect_nodes(i_mm3, "output_1", i_mm2, "input_1", millimeter!(2200.0))?;
    scenery.connect_nodes(i_mm2, "output_1", i_main_amp, "input", millimeter!(1070.0))?;

    scenery.connect_nodes(i_main_amp, "output", i_sd3, "input_1", millimeter!(0.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/workshop_09_phelix.opm"))
}

fn amp(name: &str) -> OpmResult<NodeGroup> {
    let fused_silica = RefrIndexConst::new(1.5)?;
    let mut amp = NodeGroup::new(name);
    let disk_a = Wedge::new("disk A", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(0.0, -56.0, 0.0))?;
    let i_a1 = amp.add_node(disk_a)?;

    let disk_a = Wedge::new("disk B", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(0.0, 56.0, 0.0))?;
    let i_a2 = amp.add_node(disk_a)?;

    amp.connect_nodes(i_a1, "output_1", i_a2, "input_1", millimeter!(900.0))?;
    amp.map_input_port(i_a1, "input_1", "input")?;
    amp.map_output_port(i_a2, "output_1", "output")?;
    Ok(amp)
}
