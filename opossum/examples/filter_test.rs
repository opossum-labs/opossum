use std::path::Path;

use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    error::OpmResult,
    lightdata::{DataEnergy, LightData},
    nodes::{BeamSplitter, EnergyMeter, FilterType, IdealFilter, NodeGroup, Source, Spectrometer},
    ray::SplittingConfig,
    spectrum::Spectrum,
    spectrum_helper::create_he_ne_spec,
    OpmDocument,
};
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("filter system demo");
    let i_s = scenery.add_node(Source::new(
        "Source",
        &LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0)?,
        }),
    ))?;
    let i_bs = scenery.add_node(BeamSplitter::new("bs", &SplittingConfig::Ratio(0.6)).unwrap())?;
    let filter_spectrum = Spectrum::from_csv("./opossum/opossum/NE03B.csv")?;
    let i_f = scenery.add_node(IdealFilter::new(
        "filter",
        &FilterType::Spectrum(filter_spectrum),
    )?)?;
    let i_d1 = scenery.add_node(EnergyMeter::new(
        "Energy meter 1",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ))?;
    let i_d2 = scenery.add_node(Spectrometer::default())?;
    let i_d3 = scenery.add_node(EnergyMeter::new(
        "Energy meter 2",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ))?;

    scenery.connect_nodes(&i_s, "output_1", &i_bs, "input_1", Length::zero())?;
    scenery.connect_nodes(&i_bs, "out1_trans1_refl2", &i_d1, "input_1", Length::zero())?;
    scenery.connect_nodes(&i_bs, "out2_trans2_refl1", &i_f, "input_1", Length::zero())?;
    scenery.connect_nodes(&i_f, "output_1", &i_d2, "input_1", Length::zero())?;
    scenery.connect_nodes(&i_d2, "output_1", &i_d3, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/filter_test.opm"))
}
