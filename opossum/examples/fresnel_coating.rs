use opossum::{
    OpmDocument,
    analyzers::{AnalyzerType, RayTraceConfig},
    coatings::CoatingType,
    energy_distributions::UniformDist,
    error::OpmResult,
    joule,
    lightdata::{light_data_builder::LightDataBuilder, ray_data_builder::RayDataBuilder},
    millimeter, nanometer,
    nodes::{EnergyMeter, FluenceDetector, Lens, NodeGroup, RayPropagationVisualizer, Source},
    optic_node::OpticNode,
    optic_ports::PortType,
    position_distributions::Grid,
    refractive_index::RefrIndexConst,
    spectral_distribution::LaserLines,
    utils::geom_transformation::Isometry,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Fresnel coating example");
    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated {
        pos_dist: Grid::new((millimeter!(9.), millimeter!(9.)), (100, 100))?.into(),
        energy_dist: UniformDist::new(joule!(1.))?.into(),
        spect_dist: LaserLines::new(vec![(nanometer!(1000.), 1.0)])?.into(),
    });
    let mut source = Source::new("src", light_data_builder);
    source.set_isometry(Isometry::identity())?;
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
    doc.save_to_file(Path::new("./opossum/playground/fresnel_coating.opm"))
}
