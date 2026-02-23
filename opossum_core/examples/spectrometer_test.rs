use opossum_core::prelude::*;
use opossum_core::{
    energy_distributions::UniformDist, position_distributions::Hexapolar,
    spectral_distribution::LaserLines,
};
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Raysource demo");
    let light_data_builder =
        LightDataBuilder::Geometric(RayDataSource::Collimated(CollimatedSrc::new(
            Hexapolar::new(millimeter!(1.0), 3)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(vec![
                (nanometer!(1000.0), 1.0),
                (nanometer!(800.0), 0.75),
                (nanometer!(850.0), 0.5),
            ])?
            .into(),
        )));
    let src = Source::new("collimated line ray source", light_data_builder);
    let i_src = scenery.add_node(src)?;
    let i_spec = scenery.add_node(Spectrometer::default())?;
    scenery.connect_nodes(i_src, "output_1", i_spec, "input_1", millimeter!(5.0))?;
    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(std::path::Path::new(
        "./opossum_core/playground/spectrometer.opm",
    ))?;
    Ok(())
}
