use nalgebra::Point2;
use opossum_core::core_optics::OpticNodeExt;
use opossum_core::prelude::*;
use opossum_core::{
    apertures::CircleShape, coatings::CoatingType, core_optics::PortType,
    distributions::energy::UniformDist, distributions::position::Grid,
    distributions::spectral::LaserLines,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Fresnel coating example");
    let i_src = scenery.add_node(SourcePort::new("collimated ray source"))?;

    let fd1 = scenery.add_node(FluenceDetector::new("before lens"))?;

    // Curved to 10 mm and 9 mm, the lens reaches no further than 9 mm from its axis.
    let mut lens1 = Lens::new_with_clear_aperture(
        "Lens",
        millimeter!(10.0),
        millimeter!(9.0),
        millimeter!(1.0),
        RefrIndexConst::new(1.5)?,
        CircleShape::new(millimeter!(9.0))?.into(),
    )?;
    lens1.set_coating(&PortType::Input, "input_1", &CoatingType::Fresnel)?;
    let l1 = scenery.add_node(lens1)?;
    let fd2 = scenery.add_node(FluenceDetector::new("after lens"))?;
    let ed = scenery.add_node(EnergyMeter::default())?;
    let det = scenery.add_node(RayPropagationVisualizer::default())?;

    scenery.connect_nodes(i_src, "output_1", fd1, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(fd1, "output_1", l1, "input_1", millimeter!(1.0))?;
    scenery.connect_nodes(l1, "output_1", fd2, "input_1", millimeter!(1.0))?;
    scenery.connect_nodes(fd2, "output_1", ed, "input_1", millimeter!(1.0))?;
    scenery.connect_nodes(ed, "output_1", det, "input_1", millimeter!(10.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    let ray_data_source = RayDataSource::Collimated(CollimatedSrc::new(
        Grid::new(
            Point2::new(millimeter!(9.), millimeter!(9.)),
            Point2::new(100, 100),
        )?
        .into(),
        UniformDist::new(joule!(1.))?.into(),
        LaserLines::new(vec![(nanometer!(1000.), 1.0)])?.into(),
    ));
    config.map_source(i_src, ray_data_source.into());
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new("./opossum_core/playground/fresnel_coating.opm"))
}
