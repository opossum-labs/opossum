use nalgebra::Vector3;
use opossum_core::{
    analyzers::energy::EnergyConfig,
    nodes::round_collimated_ray_builder, prelude::*,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let src = scenery.add_node(SourcePort::new("round collimated ray source"))?;
    let lens = CylindricLens::new(
        "Lens 1",
        millimeter!(100.0),
        millimeter!(f64::INFINITY),
        millimeter!(5.0),
        &RefrIndexConst::new(1.5068)?,
    )?
    .with_tilt(degree!(0.0, 0.0, 45.0))?;
    let l1 = scenery.add_node(lens)?;
    let det = scenery.add_node(RayPropagationVisualizer::new(
        "Ray_positions",
        Some(Vector3::y()),
    )?)?;
    let det2 = scenery.add_node(SpotDiagram::default())?;
    let det3=scenery.add_node(EnergyMeter::default())?;
    scenery.connect_nodes(src, "output_1", l1, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(l1, "output_1", det, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(det, "output_1", det2, "input_1", millimeter!(0.0))?;
    scenery.connect_nodes(det2, "output_1", det3, "input_1", millimeter!(0.0))?;

    let mut doc = OpmDocument::new(scenery);

    let mut config = RayTraceConfig::default();
    let ray_data_builder = round_collimated_ray_builder(millimeter!(20.0), joule!(1.0), 10)?;
    config.map_source(src, ray_data_builder.into());

    doc.add_analyzer(AnalyzerType::RayTrace(config));

    let mut config = EnergyConfig::default();
    let energy_data_builder = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
        vec![(nanometer!(633.0), joule!(1.0))],
        nanometer!(1.0),
    )?);
    config.map_source(src, energy_data_builder);
    doc.add_analyzer(AnalyzerType::Energy(config));
    doc.save_to_file(Path::new(
        "./opossum_core/playground/cylindric_lens_test.opm",
    ))
}
