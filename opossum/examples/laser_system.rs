use std::path::Path;

use opossum::{
    error::OpmResult,
    nodes::{
        create_collimated_ray_source, BeamSplitter, EnergyMeter, IdealFilter, NodeGroup,
        ParaxialSurface, Propagation, SpotDiagram,
    },
    OpticScenery, SplittingConfig,
};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::millimeter,
};

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    scenery.set_description("laser system")?;

    // Main beam line
    let i_src = scenery.add_node(create_collimated_ray_source(
        Length::new::<millimeter>(1.0),
        Energy::new::<joule>(1.0),
        3,
    )?);
    let i_l1 = scenery.add_node(ParaxialSurface::new(
        "f=100",
        Length::new::<millimeter>(100.0),
    )?);
    let i_p1 = scenery.add_node(Propagation::new("l=300", Length::new::<millimeter>(300.0))?);
    let i_l2 = scenery.add_node(ParaxialSurface::new(
        "f=200",
        Length::new::<millimeter>(200.0),
    )?);
    let i_bs = scenery.add_node(BeamSplitter::new("1% BS", &SplittingConfig::Ratio(0.99))?);
    let i_e1 = scenery.add_node(EnergyMeter::new(
        "Energy meter 1",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ));
    let i_sd1 = scenery.add_node(SpotDiagram::new("Output"));

    scenery.connect_nodes(i_src, "out1", i_l1, "front")?;
    scenery.connect_nodes(i_l1, "rear", i_p1, "front")?;
    scenery.connect_nodes(i_p1, "rear", i_l2, "front")?;
    scenery.connect_nodes(i_l2, "rear", i_bs, "input1")?;
    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_e1, "in1")?;
    scenery.connect_nodes(i_e1, "out1", i_sd1, "in1")?;

    // Diagnostic beam line
    let i_f = scenery.add_node(IdealFilter::new(
        "OD1 filter",
        opossum::nodes::FilterType::Constant(0.1),
    )?);
    scenery.connect_nodes(i_bs, "out2_trans2_refl1", i_f, "front")?;

    // Cam Box
    let mut cam_box = NodeGroup::new("CamBox");

    let i_cb_bs = cam_box.add_node(BeamSplitter::new("50/50 BS", &SplittingConfig::Ratio(0.5))?)?;
    let i_cb_l = cam_box.add_node(ParaxialSurface::new(
        "FF lens",
        Length::new::<millimeter>(100.0),
    )?)?;
    let i_cb_p = cam_box.add_node(Propagation::new("l=100", Length::new::<millimeter>(100.0))?)?;
    let i_cb_sd1 = cam_box.add_node(SpotDiagram::new("Nearfield"))?;
    let i_cb_sd2 = cam_box.add_node(SpotDiagram::new("Farfield"))?;
    let i_cb_e = cam_box.add_node(EnergyMeter::new(
        "Energy meter",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ))?;

    cam_box.connect_nodes(i_cb_bs, "out1_trans1_refl2", i_cb_l, "front")?;
    cam_box.connect_nodes(i_cb_l, "rear", i_cb_p, "front")?;
    cam_box.connect_nodes(i_cb_p, "rear", i_cb_sd2, "in1")?;

    cam_box.connect_nodes(i_cb_bs, "out2_trans2_refl1", i_cb_sd1, "in1")?;
    cam_box.connect_nodes(i_cb_sd1, "out1", i_cb_e, "in1")?;

    cam_box.map_input_port(i_cb_bs, "input1", "input")?;
    cam_box.expand_view(true)?;
    let i_cam_box = scenery.add_node(cam_box);
    scenery.connect_nodes(i_f, "rear", i_cam_box, "input")?;

    scenery.save_to_file(Path::new("./opossum/playground/laser_system.opm"))?;
    Ok(())
}
