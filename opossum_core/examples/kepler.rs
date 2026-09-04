use nalgebra::Point2;
use num_traits::Zero;
use opossum_core::{
    core_optics::OpticNodeExt,
    distributions::{energy::UniformDist, position::Grid, spectral::LaserLines},
    nodes::SourcePort,
    prelude::*,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(SourcePort::default())?;
    let mut lens1 = ParaxialSurface::new("100 mm lens", millimeter!(100.0))?;
    let aperture = Aperture::new_circle(millimeter!(25.), ApertureType::Hole, None)?;
    lens1.set_aperture(&PortType::Input, "input_1", &aperture)?;
    let i_pl1 = scenery.add_node(lens1)?;
    let i_pl2 = scenery.add_node(ParaxialSurface::new("50 mm lens", millimeter!(50.0))?)?;
    let i_sd3 = scenery.add_node(RayPropagationVisualizer::new("after telecope", None)?)?;
    scenery.connect_nodes(i_src, "output_1", i_pl1, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(150.0))?;
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(50.0))?;

    let mut doc = OpmDocument::new(scenery);

    let ray_data_builder = RayDataSource::Collimated(CollimatedSrc::new(
        Grid::new(
            Point2::new(Length::zero(), millimeter!(20.0)),
            Point2::new(1, 3),
        )?
        .into(),
        UniformDist::new(joule!(1.0))?.into(),
        LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
    ));
    let mut ray_trace_config = RayTraceConfig::default();
    ray_trace_config.map_source(i_src, ray_data_builder.into());
    doc.add_analyzer(AnalyzerType::RayTrace(ray_trace_config));
    doc.save_to_file(Path::new("./opossum_core/playground/kepler.opm"))
}
