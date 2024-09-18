use nalgebra::Vector3;
use opossum::{
    error::OpmResult,
    millimeter,
    nodes::{NodeGroup, ParaxialSurface, RayPropagationVisualizer, SpotDiagram},
};

pub fn detector_group() -> OpmResult<NodeGroup> {
    let mut cb = NodeGroup::new("Detector Group");

    let i_prop_vis_top_view = cb.add_node(RayPropagationVisualizer::new(
        "Ray_positions_top",
        Some(Vector3::y()),
    )?)?;
    let i_prop_vis_side_view = cb.add_node(RayPropagationVisualizer::new(
        "Ray_positions_side",
        Some(Vector3::x()),
    )?)?;
    let paraxial_lens = cb.add_node(ParaxialSurface::new("ideal lens", millimeter!(500.))?)?;
    let spot_monitor = SpotDiagram::new("spot diagram");
    // let rect_config = RectangleConfig::new(millimeter!(150.), millimeter!(150.), micrometer!(0.,0.))?;
    // let aperture = Aperture::BinaryRectangle(rect_config);
    // spot_monitor.set_aperture(&PortType::Input, "in1", &aperture)?;
    // spot_monitor.set_property("plot_aperture", true.into())?;
    let spot_diag = cb.add_node(spot_monitor)?;

    cb.connect_nodes(paraxial_lens, "rear", spot_diag, "in1", millimeter!(500.0))?;
    cb.connect_nodes(
        spot_diag,
        "out1",
        i_prop_vis_top_view,
        "in1",
        millimeter!(0.0),
    )?;
    cb.connect_nodes(
        i_prop_vis_top_view,
        "out1",
        i_prop_vis_side_view,
        "in1",
        millimeter!(0.0),
    )?;

    cb.add_node(RayPropagationVisualizer::new(
        "stale visualizer",
        Some(Vector3::x()),
    )?)?;

    cb.map_input_port(paraxial_lens, "front", "input")?;
    cb.map_output_port(i_prop_vis_side_view, "out1", "output")?;

    Ok(cb)
}
