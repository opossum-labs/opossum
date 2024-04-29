use num::Zero;
use opossum::{
    error::OpmResult,
    lightdata::{DataEnergy, LightData},
    nodes::{EnergyMeter, IdealFilter, NodeReference, Source},
    spectrum_helper::create_he_ne_spec,
    OpticScenery,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Reference node demo")?;
    let src = scenery.add_node(Source::new(
        "source",
        &LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0)?,
        }),
    ));
    let filt = scenery.add_node(IdealFilter::new(
        "50 % filter",
        &opossum::nodes::FilterType::Constant(0.5),
    )?);
    let reference = scenery.add_node(NodeReference::from_node(&scenery.node(filt).unwrap()));
    let detector = scenery.add_node(EnergyMeter::default());
    scenery.connect_nodes(src, "out1", filt, "front", Length::zero())?;
    scenery.connect_nodes(filt, "rear", reference, "front", Length::zero())?;
    scenery.connect_nodes(reference, "rear", detector, "in1", Length::zero())?;
    scenery.save_to_file(Path::new("./opossum/playground/reference_test.opm"))?;
    Ok(())
}
