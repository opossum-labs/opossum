use opossum_core::{
    coatings::{CoatingConstantR, CoatingType},
    core_optics::{OpticNodeExt, PortType},
    distributions::{energy::General2DGaussian, position::HexagonalTiling, spectral::LaserLines},
    percent,
    prelude::*,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Ghost Focus Example");
    let i_src = scenery.add_node(SourcePort::new("collimated ray source"))?;
    let mut lens = Lens::default();
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
    let i_l = scenery.add_node(lens)?;

    let mut lens2 = Lens::default();
    lens2.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingConstantR::new(percent!(2.0))?.into(),
    )?;
    lens2.set_coating(
        &PortType::Output,
        "output_1",
        &CoatingConstantR::new(percent!(2.0))?.into(),
    )?;
    lens2.set_coating(&PortType::Input, "input_1", &CoatingType::Fresnel)?;
    lens2.set_coating(&PortType::Output, "output_1", &CoatingType::Fresnel)?;
    let i_l2 = scenery.add_node(lens2)?;

    let mir1 = scenery.add_node(ThinMirror::new("mirror 1").with_tilt(degree!(45., 0., 0.))?)?;
    let mir2 = scenery.add_node(ThinMirror::new("mirror 2").with_tilt(degree!(45., 0., 0.))?)?;
    let mir3 = scenery.add_node(ThinMirror::new("mirror 3").with_tilt(degree!(-45., 0., 0.))?)?;
    let mir4 = scenery.add_node(ThinMirror::new("mirror 4").with_tilt(degree!(-45., 0., 0.))?)?;

    scenery.connect_nodes(i_src, "output_1", i_l, "input_1", millimeter!(150.0))?;
    scenery.connect_nodes(i_l, "output_1", mir1, "input_1", millimeter!(150.0))?;
    scenery.connect_nodes(mir1, "output_1", mir2, "input_1", millimeter!(200.0))?;
    scenery.connect_nodes(mir2, "output_1", mir3, "input_1", millimeter!(300.0))?;
    scenery.connect_nodes(mir3, "output_1", mir4, "input_1", millimeter!(200.0))?;
    scenery.connect_nodes(mir4, "output_1", i_l2, "input_1", millimeter!(150.0))?;

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
    config.set_max_bounces(1);
    doc.add_analyzer(AnalyzerType::GhostFocus(config));
    doc.save_to_file(Path::new("./opossum_core/playground/ghost_focus.opm"))
}
