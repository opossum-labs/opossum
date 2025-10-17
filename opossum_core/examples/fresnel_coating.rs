use opossum_core::prelude::*;
use opossum_core::{
    coatings::CoatingType, energy_distributions::UniformDist, optic_ports::PortType,
    position_distributions::Grid, spectral_distribution::LaserLines,
};
use std::path::Path;
use nalgebra::Point2;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Fresnel coating example");
    let light_data_builder =
        LightDataBuilder::Geometric(RayDataBuilder::Collimated(CollimatedSrc::new(
            Grid::new(Point2::new(millimeter!(9.), millimeter!(9.)), Point2::new(100, 100))?.into(),
            UniformDist::new(joule!(1.))?.into(),
            LaserLines::new(vec![(nanometer!(1000.), 1.0)])?.into(),
        )));
    let source = Source::new("src", light_data_builder);
    let src = scenery.add_node(source)?;
    let fd1 = scenery.add_node(FluenceDetector::new("before lens"))?;

    let mut lens1 = Lens::new(
        "Lens",
        millimeter!(10.0),
        millimeter!(9.0),
        millimeter!(1.0),
        &RefrIndexConst::new(1.5)?,
    )?;
    lens1.set_coating(&PortType::Input, "input_1", &CoatingType::Fresnel)?;
    let l1 = scenery.add_node(lens1)?;
    let fd2 = scenery.add_node(FluenceDetector::new("after lens"))?;
    let ed = scenery.add_node(EnergyMeter::default())?;
    let det = scenery.add_node(RayPropagationVisualizer::default())?;

    scenery.connect_nodes(src, "output_1", fd1, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(fd1, "output_1", l1, "input_1", millimeter!(1.0))?;
    scenery.connect_nodes(l1, "output_1", fd2, "input_1", millimeter!(1.0))?;
    scenery.connect_nodes(fd2, "output_1", ed, "input_1", millimeter!(1.0))?;
    scenery.connect_nodes(ed, "output_1", det, "input_1", millimeter!(10.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum_core/playground/fresnel_coating.opm"))
}
