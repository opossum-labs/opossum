use opossum::{
    aperture::{Aperture, RectangleConfig},
    error::OpmResult,
    millimeter, nanometer,
    nodes::{BeamSplitter, Dummy, FluenceDetector, Lens, NodeGroup, ParaxialSurface, SpotDiagram},
    optical::Optical,
    ray::SplittingConfig,
    refractive_index::RefrIndexSellmeier1,
};

pub fn stretcher_telescope() -> OpmResult<NodeGroup> {
    let mut cb = NodeGroup::new("Stretcher Telescope");
    let nbk7 = RefrIndexSellmeier1::new(
        1.039612120,
        0.231792344,
        1.010469450,
        0.00600069867,
        0.0200179144,
        103.5606530,
        nanometer!(300.)..nanometer!(1200.),
    )?;

    //focal length = 996.7 mm (Thorlabs LA1779-B)
    let lens_1 = cb.add_node(Lens::new(
        "Lens 1",
        millimeter!(515.1),
        millimeter!(f64::INFINITY),
        millimeter!(3.6),
        &nbk7,
    )?)?;

    //focal length = 996.7 mm (Thorlabs LA1779-B)
    let lens_2 = cb.add_node(Lens::new(
        "Lens 2",
        millimeter!(f64::INFINITY),
        millimeter!(515.1),
        millimeter!(3.6),
        &nbk7,
    )?)?;

    cb.connect_nodes(lens_1, "rear", lens_2, "front", millimeter!(996.7 * 2.))?;

    cb.map_input_port(lens_1, "front", "input")?;
    cb.map_input_port(lens_2, "rear", "output")?;
    Ok(cb)
}
