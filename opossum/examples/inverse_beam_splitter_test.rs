use num::Zero;
use opossum::{
    error::OpmResult,
    lightdata::{DataEnergy, LightData},
    nodes::{BeamSplitter, EnergyMeter, Source},
    optical::Optical,
    ray::SplittingConfig,
    spectrum_helper::create_he_ne_spec,
    OpmDocument, OpticScenery,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    scenery.set_description("inverse beam splitter test");

    let i_s = scenery.add_node(Source::new(
        "Source",
        &LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0)?,
        }),
    ));
    let mut bs = BeamSplitter::new("bs", &SplittingConfig::Ratio(0.6)).unwrap();
    bs.set_inverted(true)?;
    let i_bs = scenery.add_node(bs);
    let i_d1 = scenery.add_node(EnergyMeter::new(
        "Energy meter 1",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ));
    let i_d2 = scenery.add_node(EnergyMeter::new(
        "Energy meter 2",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ));

    scenery.connect_nodes(i_s, "out1", i_bs, "out1_trans1_refl2", Length::zero())?;
    scenery.connect_nodes(i_bs, "input1", i_d1, "in1", Length::zero())?;
    scenery.connect_nodes(i_bs, "input2", i_d2, "in1", Length::zero())?;

    OpmDocument::new(scenery)
        .save_to_file(Path::new("./opossum/playground/inverse_beam_splitter.opm"))
}
