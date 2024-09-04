#![allow(missing_docs)]
use std::path::Path;

use num::Zero;
use opossum::{
    analyzers::AnalyzerType,
    error::OpmResult,
    lightdata::{DataEnergy, LightData},
    nodes::{BeamSplitter, Detector, FilterType, IdealFilter, NodeGroup, Source},
    ray::SplittingConfig,
    spectrum::Spectrum,
    spectrum_helper::{create_he_ne_spec, create_nd_glass_spec},
    OpmDocument,
};
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("beam combiner demo");
    let i_s1 = scenery.add_node(Source::new(
        "Source 1",
        &LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        }),
    ))?;
    let i_s2 = scenery.add_node(Source::new(
        "Source 2",
        &LightData::Energy(DataEnergy {
            spectrum: create_nd_glass_spec(1.0)?,
        }),
    ))?;
    let i_bs = scenery.add_node(BeamSplitter::new("bs", &SplittingConfig::Ratio(0.5)).unwrap())?;
    let filter_spectrum = Spectrum::from_csv("./opossum/NE03B.csv")?;
    let i_f = scenery.add_node(IdealFilter::new(
        "filter",
        &FilterType::Spectrum(filter_spectrum),
    )?)?;
    let i_d1 = scenery.add_node(Detector::default())?; // Detector 1

    scenery.connect_nodes(i_s1, "out1", i_bs, "input1", Length::zero())?;
    scenery.connect_nodes(i_s2, "out1", i_bs, "input2", Length::zero())?;
    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_f, "front", Length::zero())?;
    scenery.connect_nodes(i_f, "rear", i_d1, "in1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::Energy);
    doc.save_to_file(Path::new("./opossum/playground/beam_combiner_test.opm"))
}
