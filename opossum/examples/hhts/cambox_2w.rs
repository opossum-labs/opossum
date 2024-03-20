use nalgebra::Point2;
use opossum::{
    aperture::{Aperture, RectangleConfig},
    error::OpmResult,
    nodes::{BeamSplitter, NodeGroup, ParaxialSurface, Propagation, SpotDiagram},
    optical::Optical,
    ray::SplittingConfig,
};
use uom::num_traits::Zero;
use uom::si::{f64::Length, length::millimeter};

pub fn cambox_2w() -> OpmResult<NodeGroup> {
    let config = RectangleConfig::new(
        Length::new::<millimeter>(11.33),
        Length::new::<millimeter>(7.13),
        Point2::new(Length::zero(), Length::zero()),
    )?;
    let cam_aperture = Aperture::BinaryRectangle(config);

    let mut cb = NodeGroup::new("CamBox 2w");

    let d1 = cb.add_node(Propagation::new("d1", Length::new::<millimeter>(35.0))?)?;
    let bs1 = cb.add_node(BeamSplitter::new("bs1", &SplittingConfig::Ratio(0.5))?)?;

    cb.connect_nodes(d1, "rear", bs1, "input1")?;

    // FF path
    let ff_d1 = cb.add_node(Propagation::new("ff_d1", Length::new::<millimeter>(100.0))?)?;
    let bs_ff = cb.add_node(BeamSplitter::new("bs_ff", &SplittingConfig::Ratio(0.04))?)?;
    let ff_d2 = cb.add_node(Propagation::new("ff_d2", Length::new::<millimeter>(25.0))?)?;
    let ff_lens = cb.add_node(ParaxialSurface::new(
        "FF lens",
        Length::new::<millimeter>(100.0),
    )?)?;
    let ff_d3 = cb.add_node(Propagation::new("ff_d3", Length::new::<millimeter>(100.0))?)?;
    let mut node = SpotDiagram::new("FF cam");
    node.set_input_aperture("in1", &cam_aperture)?;
    let ff_cam = cb.add_node(node)?;

    cb.connect_nodes(bs1, "out1_trans1_refl2", ff_d1, "front")?;
    cb.connect_nodes(ff_d1, "rear", bs_ff, "input1")?;
    cb.connect_nodes(bs_ff, "out1_trans1_refl2", ff_d2, "front")?;
    cb.connect_nodes(ff_d2, "rear", ff_lens, "front")?;
    cb.connect_nodes(ff_lens, "rear", ff_d3, "front")?;
    cb.connect_nodes(ff_d3, "rear", ff_cam, "in1")?;

    // NF path
    let nf_d1 = cb.add_node(Propagation::new("nf_d1", Length::new::<millimeter>(35.0))?)?;
    let nf_lens1 = cb.add_node(ParaxialSurface::new(
        "NF lens1",
        Length::new::<millimeter>(125.0),
    )?)?;
    let nf_d2 = cb.add_node(Propagation::new("nf_d1", Length::new::<millimeter>(250.0))?)?;
    let nf_lens2 = cb.add_node(ParaxialSurface::new(
        "NF lens2",
        Length::new::<millimeter>(125.0),
    )?)?;
    let nf_d3 = cb.add_node(Propagation::new("nf_d1", Length::new::<millimeter>(50.0))?)?;
    let nf_bs = cb.add_node(BeamSplitter::new("nf bs", &SplittingConfig::Ratio(0.5))?)?;
    let nf_d4 = cb.add_node(Propagation::new("nf_d4", Length::new::<millimeter>(130.0))?)?;
    let mut node = SpotDiagram::new("NF cam");
    node.set_input_aperture("in1", &cam_aperture)?;
    let nf_cam = cb.add_node(node)?;

    cb.connect_nodes(bs1, "out2_trans2_refl1", nf_d1, "front")?;
    cb.connect_nodes(nf_d1, "rear", nf_lens1, "front")?;
    cb.connect_nodes(nf_lens1, "rear", nf_d2, "front")?;
    cb.connect_nodes(nf_d2, "rear", nf_lens2, "front")?;
    cb.connect_nodes(nf_lens2, "rear", nf_d3, "front")?;
    cb.connect_nodes(nf_d3, "rear", nf_bs, "input1")?;
    cb.connect_nodes(nf_bs, "out1_trans1_refl2", nf_d4, "front")?;
    cb.connect_nodes(nf_d4, "rear", nf_cam, "in1")?;

    cb.map_input_port(d1, "front", "input")?;
    Ok(cb)
}
