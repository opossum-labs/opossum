use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{
        round_collimated_ray_source, BeamSplitter, EnergyMeter, IdealFilter, NodeGroup,
        ParaxialSurface, SpotDiagram,
    },
    ray::SplittingConfig,
    OpmDocument,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("laser system");
    // Main beam line

    // let source = Source::new(
    //     "Source",
    //     &LightData::Energy(DataEnergy {
    //         spectrum: create_he_ne_spec(1.0)?,
    //     }),
    // );
    let source = round_collimated_ray_source(millimeter!(1.0), joule!(1.0), 3)?;
    let i_src = scenery.add_node(source)?;
    let i_l1 = scenery.add_node(ParaxialSurface::new("f=100", millimeter!(100.0))?)?;
    let i_l2 = scenery.add_node(ParaxialSurface::new("f=200", millimeter!(200.0))?)?;
    let i_bs = scenery.add_node(BeamSplitter::new("1% BS", &SplittingConfig::Ratio(0.99))?)?;
    let i_e1 = scenery.add_node(EnergyMeter::new(
        "Energy meter 1",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ))?;
    let i_sd1 = scenery.add_node(SpotDiagram::new("output_1"))?;

    scenery.connect_nodes(i_src, "output_1", i_l1, "input_1", Length::zero())?;
    scenery.connect_nodes(i_l1, "output_1", i_l2, "input_1", millimeter!(300.0))?;
    scenery.connect_nodes(i_l2, "output_1", i_bs, "input_1", Length::zero())?;
    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_e1, "input_1", Length::zero())?;
    scenery.connect_nodes(i_e1, "output_1", i_sd1, "input_1", Length::zero())?;

    // Diagnostic beam line
    let i_f = scenery.add_node(IdealFilter::new(
        "OD1 filter",
        &opossum::nodes::FilterType::Constant(0.1),
    )?)?;
    scenery.connect_nodes(i_bs, "out2_trans2_refl1", i_f, "input_1", Length::zero())?;

    // Cam Box
    let mut cam_box = NodeGroup::new("CamBox");

    let i_cb_bs = cam_box.add_node(BeamSplitter::new("50/50 BS", &SplittingConfig::Ratio(0.5))?)?;
    let i_cb_l = cam_box.add_node(ParaxialSurface::new("FF lens", millimeter!(100.0))?)?;
    let i_cb_sd1 = cam_box.add_node(SpotDiagram::new("Nearfield"))?;
    let i_cb_sd2 = cam_box.add_node(SpotDiagram::new("Farfield"))?;
    let i_cb_e = cam_box.add_node(EnergyMeter::new(
        "Energy meter",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ))?;

    cam_box.connect_nodes(
        i_cb_bs,
        "out1_trans1_refl2",
        i_cb_l,
        "input_1",
        Length::zero(),
    )?;
    cam_box.connect_nodes(i_cb_l, "output_1", i_cb_sd2, "input_1", millimeter!(100.0))?;

    cam_box.connect_nodes(
        i_cb_bs,
        "out2_trans2_refl1",
        i_cb_sd1,
        "input_1",
        Length::zero(),
    )?;
    cam_box.connect_nodes(i_cb_sd1, "output_1", i_cb_e, "input_1", Length::zero())?;

    cam_box.map_input_port(&i_cb_bs, "input_1", "input_1")?;
    let i_cam_box = scenery.add_node(cam_box)?;
    scenery.connect_nodes(i_f, "output_1", i_cam_box, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/laser_system.opm"))
}
