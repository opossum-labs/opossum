use opossum_core::prelude::*;
use opossum_core::{
    J_per_cm2, coatings::CoatingType, energy_distributions::General2DGaussian,
    position_distributions::HexagonalTiling, spectral_distribution::LaserLines,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Ghostfocus demo");

    let mut src = SourcePort::new("collimated ray source");
    src.node_attr_mut().set_lidt(&J_per_cm2!(2.0))?;
    let i_src = scenery.add_node(src)?;

    let mir1 = scenery.add_node(ThinMirror::new("Mirror 1").with_tilt(degree!(45., 0., 0.))?)?;

    let mut lens = Lens::default();
    lens.node_attr_mut().set_name("Lens 1");
    lens.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingType::ConstantR { reflectivity: 0.05 },
    )?;
    lens.set_coating(
        &PortType::Output,
        "output_1",
        &CoatingType::ConstantR { reflectivity: 0.05 },
    )?;
    lens.node_attr_mut().set_lidt(&J_per_cm2!(2.0))?;
    let i_l = scenery.add_node(lens)?;

    let mir2 = scenery.add_node(ThinMirror::new("Mirror 2").with_tilt(degree!(45., 0., 0.))?)?;

    let mut lens2 = Lens::default();
    lens2.node_attr_mut().set_name("Lens 2");
    lens2.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingType::ConstantR { reflectivity: 0.2 },
    )?;
    lens2.set_coating(
        &PortType::Output,
        "output_1",
        &CoatingType::ConstantR { reflectivity: 0.2 },
    )?;
    lens2.set_coating(&PortType::Input, "input_1", &CoatingType::Fresnel)?;
    lens2.set_coating(&PortType::Output, "output_1", &CoatingType::Fresnel)?;
    lens2.node_attr_mut().set_lidt(&J_per_cm2!(2.0))?;
    let i_l2 = scenery.add_node(lens2)?;

    let mir3 = scenery.add_node(ThinMirror::new("Mirror 3"))?; // .with_tilt(degree!(5., 0., 0.))?)?;

    scenery.connect_nodes(i_src, "output_1", mir1, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(mir1, "output_1", i_l, "input_1", millimeter!(125.0))?;
    scenery.connect_nodes(i_l, "output_1", mir2, "input_1", millimeter!(65.0))?;
    scenery.connect_nodes(mir2, "output_1", i_l2, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_l2, "output_1", mir3, "input_1", millimeter!(150.0))?;

    let mut doc = OpmDocument::new(scenery);
    let ray_data_source = RayDataSource::Collimated(CollimatedSrc::new(
        HexagonalTiling::new(millimeter!(15.0), 25, millimeter!(0.0, 0.))?.into(),
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
    doc.save_to_file(Path::new(
        "./opossum_core/playground/workshop_11_ghostfocus.opm",
    ))
}
