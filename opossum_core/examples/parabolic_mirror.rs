use nalgebra::Vector3;
use opossum_core::{
    apertures::CircleShape, core_optics::NodeAttrExt, geometry::body::CLEAR_APERTURE,
    nodes::round_collimated_ray_builder, prelude::*,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(SourcePort::new("collimated ray source"))?;
    // The clear aperture of a parabola is measured parallel to its parent axis, here parallel to
    // the collimated beam: parabola 1 takes the beam of 240 mm radius, parabola 2 sends it on with
    // 240 mm * 50 / 400 = 30 mm radius.
    let mut parabola1 = ParabolicMirror::new_with_off_axis_y(
        "parabola 1",
        millimeter!(400.0),
        false,
        degree!(45.0),
    )?;
    parabola1.set_property(
        CLEAR_APERTURE,
        ApertureShape::from(CircleShape::new(millimeter!(250.0))?).into(),
    )?;
    let i_m1 = scenery.add_node(parabola1)?;
    let mut parabola2 = ParabolicMirror::new_with_off_axis_y(
        "parabola 2",
        millimeter!(50.0),
        true,
        degree!(-45.0),
    )?;
    parabola2.set_property(
        CLEAR_APERTURE,
        ApertureShape::from(CircleShape::new(millimeter!(38.1))?).into(),
    )?;
    let i_m2 = scenery.add_node(parabola2)?;
    let rpv =
        RayPropagationVisualizer::new("visualizer", Some(Vector3::new(10., 0., 0.).normalize()))?;
    let i_prop_vis = scenery.add_node(rpv)?;
    let i_sd = scenery.add_node(SpotDiagram::default())?;
    let i_wf = scenery.add_node(WaveFront::default())?;
    let i_pm = scenery.add_node(EnergyMeter::default())?;
    scenery.connect_nodes(i_src, "output_1", i_m1, "input_1", millimeter!(500.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(450.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_sd, "input_1", millimeter!(500.0))?;
    scenery.connect_nodes(i_sd, "output_1", i_wf, "input_1", millimeter!(0.1))?;
    scenery.connect_nodes(i_wf, "output_1", i_pm, "input_1", millimeter!(0.1))?;
    scenery.connect_nodes(i_pm, "output_1", i_prop_vis, "input_1", millimeter!(0.1))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        round_collimated_ray_builder(millimeter!(240.0), joule!(1.0), 8)?,
    );
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new("./opossum_core/playground/parabolic_mirror.opm"))
}
