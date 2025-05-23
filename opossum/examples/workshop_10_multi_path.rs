use opossum::{
    analyzers::AnalyzerType,
    error::OpmResult,
    joule,
    lightdata::{energy_data_builder::EnergyDataBuilder, light_data_builder::LightDataBuilder},
    millimeter, nanometer,
    nodes::{BeamSplitter, NodeGroup, Source, Spectrometer, SpectrometerType},
    ray::SplittingConfig,
    spectrum_helper::{generate_filter_spectrum, FilterType},
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Multi Path / Multi Source");
    let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
        vec![
            (nanometer!(1000.0), joule!(0.5)),
            (nanometer!(950.0), joule!(0.3)),
            (nanometer!(1050.0), joule!(0.2)),
        ],
        nanometer!(0.5),
    ));
    let src = Source::new("multi line source", light_data_builder);
    let i_src = scenery.add_node(src)?;
    let i_s1 = scenery.add_node(Spectrometer::new(
        "Source 1",
        SpectrometerType::Ideal,
    ))?;

    let splitting_config_1 = SplittingConfig::Spectrum(generate_filter_spectrum(
        nanometer!(800.0)..nanometer!(1100.0),
        nanometer!(0.5),
        &FilterType::LongPassStep {
            cut_off: nanometer!(980.0),
        },
    )?);
    let bs1 = BeamSplitter::new("BS1", &splitting_config_1)?;
    let i_bs1 = scenery.add_node(bs1)?;

    let i_s2 = scenery.add_node(Spectrometer::new(
        "BS1 Output 1",
        SpectrometerType::Ideal,
    ))?;

    let i_s3 = scenery.add_node(Spectrometer::new(
        "BS1 Output 2",
        SpectrometerType::Ideal,
    ))?;

    let splitting_config_2 = SplittingConfig::Spectrum(generate_filter_spectrum(
        nanometer!(800.0)..nanometer!(1100.0),
        nanometer!(0.5),
        &FilterType::LongPassStep {
            cut_off: nanometer!(1025.0),
        },
    )?);
    let bs2 = BeamSplitter::new("BS2", &splitting_config_2)?;
    let i_bs2 = scenery.add_node(bs2)?;

    let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
        vec![(nanometer!(1020.0), joule!(1.0))],
        nanometer!(0.5),
    ));
    let src2 = Source::new("source 2", light_data_builder);
    let i_src2 = scenery.add_node(src2)?;

    let i_s4 = scenery.add_node(Spectrometer::new(
        "BS2 Output 1",
        SpectrometerType::Ideal,
    ))?;

    let i_s5 = scenery.add_node(Spectrometer::new(
        "BS2 Output 2",
        SpectrometerType::Ideal,
    ))?;

    scenery.connect_nodes(i_src, "output_1", i_s1, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_s1, "output_1", i_bs1, "input_1", millimeter!(100.0))?;

    scenery.connect_nodes(
        i_bs1,
        "out1_trans1_refl2",
        i_s2,
        "input_1",
        millimeter!(100.0),
    )?;
    scenery.connect_nodes(
        i_bs1,
        "out2_trans2_refl1",
        i_s3,
        "input_1",
        millimeter!(100.0),
    )?;

    scenery.connect_nodes(i_s2, "output_1", i_bs2, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_src2, "output_1", i_bs2, "input_2", millimeter!(100.0))?;

    scenery.connect_nodes(
        i_bs2,
        "out1_trans1_refl2",
        i_s4,
        "input_1",
        millimeter!(100.0),
    )?;
    scenery.connect_nodes(
        i_bs2,
        "out2_trans2_refl1",
        i_s5,
        "input_1",
        millimeter!(100.0),
    )?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::Energy);
    doc.save_to_file(Path::new("./opossum/playground/workshop_10_multi_path.opm"))
}
