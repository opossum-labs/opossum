use opossum_core::prelude::*;
use opossum_core::{
    distributions::energy::UniformDist, distributions::position::Hexapolar,
    distributions::spectral::LaserLines,
};
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Spectrometer demo");
    let i_src = scenery.add_node(SourcePort::default())?;
    let i_spec = scenery.add_node(Spectrometer::default())?;
    scenery.connect_nodes(i_src, "output_1", i_spec, "input_1", millimeter!(5.0))?;
    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    let ray_data_source = RayDataSource::Collimated(CollimatedSrc::new(
        Hexapolar::new(millimeter!(1.0), 3)?.into(),
        UniformDist::new(joule!(1.0))?.into(),
        LaserLines::new(vec![
            (nanometer!(1000.0), 1.0),
            (nanometer!(800.0), 0.75),
            (nanometer!(850.0), 0.5),
        ])?
        .into(),
    ));
    config.map_source(i_src, ray_data_source.into());
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(std::path::Path::new(
        "./opossum_core/playground/spectrometer.opm",
    ))?;
    Ok(())
}
