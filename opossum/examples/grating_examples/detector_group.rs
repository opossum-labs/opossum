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
    let spot_diag = cb.add_node(spot_monitor)?;

    cb.connect_nodes(
        paraxial_lens,
        "output_1",
        spot_diag,
        "input_1",
        millimeter!(500.0),
    )?;
    cb.connect_nodes(
        spot_diag,
        "output_1",
        i_prop_vis_top_view,
        "input_1",
        millimeter!(0.0),
    )?;
    cb.connect_nodes(
        i_prop_vis_top_view,
        "output_1",
        i_prop_vis_side_view,
        "input_1",
        millimeter!(0.0),
    )?;

    cb.add_node(RayPropagationVisualizer::new(
        "stale visualizer",
        Some(Vector3::x()),
    )?)?;

    cb.map_input_port(paraxial_lens, "input_1", "input_1")?;
    cb.map_output_port(i_prop_vis_side_view, "output_1", "output_1")?;

    Ok(cb)
}
