use opossum::error::OpossumError;
use opossum::lightdata::{LightData, DataEnergy};
use opossum::nodes::{NodeReference, Source, IdealFilter, EnergyMeter};
use opossum::OpticScenery;
use opossum::spectrum::create_he_ne_spectrum;
use std::path::Path;

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Reference node demo");
    let src=scenery.add_node(Source::new("source", LightData::Energy(DataEnergy{spectrum: create_he_ne_spectrum(1.0)})));
    let filt = scenery.add_node(IdealFilter::new("50 % filter", opossum::nodes::FilterType::Constant(0.5))?);
    let reference = scenery.add_node(NodeReference::from_node(scenery.node(filt).unwrap()));
    let detector=scenery.add_node(EnergyMeter::default());
    scenery.connect_nodes(src, "out1", filt, "front")?;
    scenery.connect_nodes(filt,"rear", reference, "front")?;
    scenery.connect_nodes(reference, "rear", detector, "in1")?;
    scenery.save_to_file(Path::new("playground/reference_test.opm"))?;
    Ok(())
}
