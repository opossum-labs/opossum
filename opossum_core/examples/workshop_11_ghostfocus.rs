use opossum_core::coatings::CoatingConstantR;
use opossum_core::{
    J_per_cm2, coatings::CoatingType, distributions::energy::General2DGaussian,
    distributions::position::HexagonalTiling, distributions::spectral::LaserLines,
};
use opossum_core::{percent, prelude::*};
use std::env;
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Ghostfocus demo");

    let mut src = SourcePort::new("collimated ray source");
    src.set_lidt(&PortType::Output, "output_1", J_per_cm2!(2.0))?;
    let i_src = scenery.add_node(src)?;

    let mir1 = scenery.add_node(ThinMirror::new("Mirror 1").with_tilt(degree!(45., 0., 0.))?)?;

    let mut lens = Lens::default();
    lens.node_attr_mut().set_name("Lens 1");
    lens.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingConstantR::new(percent!(5.0))?.into(),
    )?;
    lens.set_coating(
        &PortType::Output,
        "output_1",
        &CoatingConstantR::new(percent!(5.0))?.into(),
    )?;
    lens.set_lidt(&PortType::Input, "input_1", J_per_cm2!(2.0))?;
    lens.set_lidt(&PortType::Output, "output_1", J_per_cm2!(2.0))?;
    let i_l = scenery.add_node(lens)?;

    let mir2 = scenery.add_node(ThinMirror::new("Mirror 2").with_tilt(degree!(45., 0., 0.))?)?;

    let mut lens2 = Lens::default();
    lens2.node_attr_mut().set_name("Lens 2");
    lens2.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingConstantR::new(percent!(20.0))?.into(),
    )?;
    lens2.set_coating(
        &PortType::Output,
        "output_1",
        &CoatingConstantR::new(percent!(20.0))?.into(),
    )?;
    lens2.set_coating(&PortType::Input, "input_1", &CoatingType::Fresnel)?;
    lens2.set_coating(&PortType::Output, "output_1", &CoatingType::Fresnel)?;
    lens2.set_lidt(&PortType::Input, "input_1", J_per_cm2!(2.0))?;
    lens2.set_lidt(&PortType::Output, "output_1", J_per_cm2!(2.0))?;
    let i_l2 = scenery.add_node(lens2)?;

    let mir3 = scenery.add_node(ThinMirror::new("Mirror 3"))?; // .with_tilt(degree!(5., 0., 0.))?)?;

    scenery.connect_nodes(i_src, "output_1", mir1, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(mir1, "output_1", i_l, "input_1", millimeter!(125.0))?;
    scenery.connect_nodes(i_l, "output_1", mir2, "input_1", millimeter!(65.0))?;
    scenery.connect_nodes(mir2, "output_1", i_l2, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_l2, "output_1", mir3, "input_1", millimeter!(150.0))?;

    let mut doc = OpmDocument::new(scenery);
    let ray_data_source = RayDataSource::Collimated(CollimatedSrc::new(
        HexagonalTiling::new(millimeter!(15.0), 25)?.into(),
        General2DGaussian::new(
            joule!(2.),
            millimeter!(0., 0.),
            millimeter!(8., 8.),
            5.,
            radian!(0.),
            false,
        )?
        .into(),
        LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
    ));
    let mut config = GhostFocusConfig::default();
    config.map_source(i_src, ray_data_source.into());
    config.set_max_bounces(2);
    doc.add_analyzer(AnalyzerType::GhostFocus(config));

    // Read the output directory from the environment, fallback to playground
    let out_dir = env::var("OPOSSUM_EXAMPLES_OUT_DIR")
        .unwrap_or_else(|_| "./opossum_core/playground".to_string());
    let out_path = Path::new(&out_dir).join("workshop_11_ghostfocus.opm");

    // Save the complete optical system to a file.
    // This file can be reopened in the framework for visualization or analysis.
    doc.save_to_file(&out_path)
}
