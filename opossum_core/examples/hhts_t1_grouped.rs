use opossum_core::coatings::CoatingType;
use opossum_core::{J_per_cm2, prelude::*};
use opossum_core::{
    distributions::energy::General2DGaussian, distributions::position::HexagonalTiling,
    distributions::spectral::LaserLines, refractive_index::refr_index_schott::RefrIndexSchott,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let refr_index_hk9l = RefrIndexSellmeier1::new(
        6.14555251E-1,
        6.56775017E-1,
        1.02699346E+0,
        1.45987884E-2,
        2.87769588E-3,
        1.07653051E+2,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    let refr_index_hzf2 = RefrIndexSellmeier1::new(
        1.67643380E-001,
        1.54335076E+000,
        1.17313123E+000,
        6.05177711E-002,
        1.18524273E-002,
        1.13671100E+002,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    // coatings
    let ar_coating = CoatingType::ConstantR { reflectivity: 0.01 };
    // apertures
    let a_2inch = Aperture::new_circle(millimeter!(25.4), millimeter!(0., 0.), ApertureType::Hole)?;

    let mut scenery = NodeGroup::new("HHT Sensor Telescope T1");
    let src = scenery.add_node(SourcePort::new("Collimated Source"))?;
    let mut telescope = NodeGroup::new("HHT Sensor Telescope T1");

    let t1_l1a = telescope.add_node(Lens::new(
        "T1 L1a",
        millimeter!(518.34008),
        millimeter!(-847.40402),
        millimeter!(30.0),
        &refr_index_hk9l,
    )?)?;
    let t1_l1b = telescope.add_node(Lens::new(
        "T1 L1b",
        millimeter!(-788.45031),
        millimeter!(-2551.88619),
        millimeter!(21.66602),
        &refr_index_hzf52,
    )?)?;
    let mut node = Lens::new(
        "T1 L2a",
        millimeter!(-88.51496),
        millimeter!(f64::INFINITY),
        millimeter!(5.77736),
        &refr_index_hzf52,
    )?;
    node.set_coating(&PortType::Input, "input_1", &ar_coating)?;
    node.set_lidt(&PortType::Input, "input_1", J_per_cm2!(0.1))?;
    node.set_lidt(&PortType::Output, "output_1", J_per_cm2!(0.1))?;
    node.set_aperture(&PortType::Input, "input_1", &a_2inch)?;
    let t1_l2a = telescope.add_node(node)?;

    let mut node = Lens::new(
        "T1 L2b",
        millimeter!(76.76954),
        millimeter!(-118.59590),
        millimeter!(14.0),
        &refr_index_hzf52,
    )?;
    node.set_coating(&PortType::Input, "input_1", &ar_coating)?;
    node.set_lidt(&PortType::Input, "input_1", J_per_cm2!(0.1))?;
    node.set_lidt(&PortType::Output, "output_1", J_per_cm2!(0.1))?;
    node.set_aperture(&PortType::Input, "input_1", &a_2inch)?;
    let t1_l2b = telescope.add_node(node)?;
    let mut node = Lens::new(
        "T1 L2c",
        millimeter!(-63.45837),
        millimeter!(66.33014),
        millimeter!(7.68327),
        &refr_index_hzf2,
    )?;
    node.set_coating(&PortType::Input, "input_1", &ar_coating)?;
    node.set_lidt(&PortType::Input, "input_1", J_per_cm2!(0.1))?;
    node.set_lidt(&PortType::Output, "output_1", J_per_cm2!(0.1))?;
    node.set_aperture(&PortType::Input, "input_1", &a_2inch)?;
    let t1_l2c = telescope.add_node(node)?;

    telescope.connect_nodes(t1_l1a, "output_1", t1_l1b, "input_1", millimeter!(10.0))?;
    telescope.connect_nodes(
        t1_l1b,
        "output_1",
        t1_l2a,
        "input_1",
        millimeter!(937.23608),
    )?;
    telescope.connect_nodes(t1_l2a, "output_1", t1_l2b, "input_1", millimeter!(8.85423))?;
    telescope.connect_nodes(t1_l2b, "output_1", t1_l2c, "input_1", millimeter!(14.78269))?;

    // collimated source definition
    let ray_data_source = RayDataSource::Collimated(CollimatedSrc::new(
        HexagonalTiling::new(millimeter!(100.), 9)?.into(),
        General2DGaussian::new(
            joule!(5.0),
            millimeter!(0., 0.),
            millimeter!(60.6389113608, 60.6389113608),
            5.,
            radian!(0.),
            false,
        )?
        .into(),
        LaserLines::new(vec![(nanometer!(1053.0), 1.0)])?.into(),
    ));

    telescope.map_input_port(t1_l1a, "input_1", "input_1")?;
    telescope.map_output_port(t1_l2c, "output_1", "output_1")?;
    let tel = scenery.add_node(telescope)?;
    scenery.connect_nodes(src, "output_1", tel, "input_1", millimeter!(100.0))?;

    // Ray propagation visualization
    let mut rpv = RayPropagationVisualizer::default();
    rpv.set_lidt(&PortType::Input, "input_1", J_per_cm2!(100.0))?;
    rpv.set_lidt(&PortType::Output, "output_1", J_per_cm2!(100.0))?;
    let rpv = scenery.add_node(rpv)?;
    scenery.connect_nodes(tel, "output_1", rpv, "input_1", millimeter!(100.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(src, ray_data_source.clone().into());
    doc.add_analyzer(AnalyzerType::RayTrace(config));

    let mut config = GhostFocusConfig::default();
    config.map_source(src, ray_data_source.into());
    assert!(config.get_source(&src).is_some());
    config.set_max_bounces(1);
    doc.add_analyzer(AnalyzerType::GhostFocus(config));
    doc.save_to_file(Path::new("./opossum_core/playground/hhts_t1_grouped.opm"))
}
