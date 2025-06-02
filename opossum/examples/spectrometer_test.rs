use opossum::{
    OpmDocument,
    analyzers::{AnalyzerType, RayTraceConfig},
    energy_distributions::UniformDist,
    error::OpmResult,
    joule,
    lightdata::{light_data_builder::LightDataBuilder, ray_data_builder::RayDataBuilder},
    millimeter, nanometer,
    nodes::{NodeGroup, Source, Spectrometer},
    optic_node::OpticNode,
    position_distributions::Hexapolar,
    spectral_distribution::LaserLines,
    utils::geom_transformation::Isometry,
};

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Raysource demo");
    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated {
        pos_dist: Hexapolar::new(millimeter!(1.0), 3)?.into(),
        energy_dist: UniformDist::new(joule!(1.0))?.into(),
        spect_dist: LaserLines::new(vec![
            (nanometer!(1000.0), 1.0),
            (nanometer!(800.0), 0.75),
            (nanometer!(850.0), 0.5),
        ])?
        .into(),
    });
    let mut src = Source::new("collimated line ray source", light_data_builder);
    src.set_isometry(Isometry::identity())?;
    let i_src = scenery.add_node(src)?;
    let i_spec = scenery.add_node(Spectrometer::default())?;
    scenery.connect_nodes(i_src, "output_1", i_spec, "input_1", millimeter!(5.0))?;
    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(std::path::Path::new(
        "./opossum/playground/spectrometer.opm",
    ))?;
    Ok(())
}
