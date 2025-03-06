use opossum::{
    aperture::{Aperture, RectangleConfig},
    error::OpmResult,
    millimeter,
    nodes::{BeamSplitter, Dummy, FluenceDetector, NodeGroup, ParaxialSurface, SpotDiagram},
    optic_node::OpticNode,
    optic_ports::PortType,
    ray::SplittingConfig,
};

pub fn cambox_1w() -> OpmResult<NodeGroup> {
    let config = RectangleConfig::new(millimeter!(11.33), millimeter!(7.13), millimeter!(0., 0.))?;
    let cam_aperture = Aperture::BinaryRectangle(config);

    let mut cb = NodeGroup::new("CamBox 1w");

    let d1 = cb.add_node(Dummy::new("d1"))?;
    let bs1 = cb.add_node(BeamSplitter::new("bs1", &SplittingConfig::Ratio(0.5))?)?;

    cb.connect_nodes(d1, "output_1", bs1, "input_1", millimeter!(35.0))?;

    // FF path
    let bs_ff = cb.add_node(BeamSplitter::new("bs_ff", &SplittingConfig::Ratio(0.04))?)?;
    let ff_lens = cb.add_node(ParaxialSurface::new("FF lens", millimeter!(100.0))?)?;
    let mut node = SpotDiagram::new("FF cam");
    node.set_aperture(&PortType::Input, "input_1", &cam_aperture)?;
    let ff_cam = cb.add_node(node)?;

    let mut ff_fluence = FluenceDetector::new("FF fluence");
    ff_fluence.set_aperture(&PortType::Input, "input_1", &cam_aperture)?;
    let ff_fluence_cam = cb.add_node(ff_fluence)?;

    cb.connect_nodes(
        bs1,
        "out1_trans1_refl2",
        bs_ff,
        "input_1",
        millimeter!(100.0),
    )?;
    cb.connect_nodes(
        bs_ff,
        "out1_trans1_refl2",
        ff_lens,
        "input_1",
        millimeter!(25.0),
    )?;
    cb.connect_nodes(ff_lens, "output_1", ff_cam, "input_1", millimeter!(100.0))?;
    cb.connect_nodes(
        ff_cam,
        "output_1",
        ff_fluence_cam,
        "input_1",
        millimeter!(0.0),
    )?;

    // NF path
    let nf_lens1 = cb.add_node(ParaxialSurface::new("NF lens1", millimeter!(125.0))?)?;
    let nf_lens2 = cb.add_node(ParaxialSurface::new("NF lens2", millimeter!(125.0))?)?;
    let nf_bs = cb.add_node(BeamSplitter::new("nf bs", &SplittingConfig::Ratio(0.5))?)?;
    let mut node = SpotDiagram::new("NF cam");
    node.set_aperture(&PortType::Input, "input_1", &cam_aperture)?;
    node.set_property("plot_aperture", true.into())?;
    let nf_cam = cb.add_node(node)?;

    let mut nf_fluence = FluenceDetector::new("NF fluence");
    nf_fluence.set_aperture(&PortType::Input, "input_1", &cam_aperture)?;
    let nf_fluence_cam = cb.add_node(nf_fluence)?;

    cb.connect_nodes(
        bs1,
        "out2_trans2_refl1",
        nf_lens1,
        "input_1",
        millimeter!(35.0),
    )?;
    cb.connect_nodes(
        nf_lens1,
        "output_1",
        nf_lens2,
        "input_1",
        millimeter!(250.0),
    )?;
    cb.connect_nodes(nf_lens2, "output_1", nf_bs, "input_1", millimeter!(50.0))?;
    cb.connect_nodes(
        nf_bs,
        "out1_trans1_refl2",
        nf_cam,
        "input_1",
        millimeter!(130.0),
    )?;

    cb.connect_nodes(
        nf_cam,
        "output_1",
        nf_fluence_cam,
        "input_1",
        millimeter!(0.0),
    )?;

    cb.map_input_port(&d1, "input_1", "input_1")?;
    Ok(cb)
}
