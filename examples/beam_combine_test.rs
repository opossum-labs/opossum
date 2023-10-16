use std::path::Path;

use opossum::{
    error::OpossumError,
    lightdata::{DataEnergy, LightData},
    nodes::{BeamSplitter, Detector, FilterType, IdealFilter, Source},
    spectrum::{create_he_ne_spectrum, create_nd_glass_spectrum, Spectrum},
    OpticScenery,
};

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("beam combiner demo");

    let i_s1 = scenery.add_node(Source::new(
        "Source 1",
        LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        }),
    ));
    let i_s2 = scenery.add_node(Source::new(
        "Source 2",
        LightData::Energy(DataEnergy {
            spectrum: create_nd_glass_spectrum(1.0),
        }),
    ));
    let i_bs = scenery.add_node(BeamSplitter::new("bs", 0.5).unwrap());
    let filter_spectrum = Spectrum::from_csv("NE03B.csv")?;
    let i_f = scenery.add_node(IdealFilter::new(
        "filter",
        FilterType::Spectrum(filter_spectrum),
    )?);
    let i_d1 = scenery.add_node(Detector::default()); // Detector 1

    scenery.connect_nodes(i_s1, "out1", i_bs, "input1")?;
    scenery.connect_nodes(i_s2, "out1", i_bs, "input2")?;
    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_f, "front")?;
    scenery.connect_nodes(i_f, "rear", i_d1, "in1")?;
    scenery.save_to_file(Path::new("playground/beam_combiner_test.opm"))?;

    Ok(())
}
