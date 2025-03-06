use num::Zero;
use opossum::{
    analyzers::AnalyzerType,
    error::OpmResult,
    lightdata::{DataEnergy, LightData},
    nodes::{BeamSplitter, Dummy, NodeGroup, NodeReference, Source},
    spectrum_helper::create_he_ne_spec,
    OpmDocument,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Michaelson interferomater");
    let src = scenery.add_node(Source::new(
        "Source",
        &LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0)?,
        }),
    ))?;
    let bs = scenery.add_node(BeamSplitter::default())?;
    let sample = scenery.add_node(Dummy::new("Sample"))?;
    let rf = NodeReference::from_node(&scenery.node(&sample)?);
    let r_sample = scenery.add_node(rf)?;
    let m1 = scenery.add_node(Dummy::new("Mirror"))?;
    let m2 = scenery.add_node(Dummy::new("Mirror"))?;
    let rf = NodeReference::from_node(&scenery.node(&bs)?);
    let r_bs = scenery.add_node(rf)?;
    let det = scenery.add_node(Dummy::new("Detector"))?;

    scenery.connect_nodes(src, "output_1", bs, "input_1", Length::zero())?;
    scenery.connect_nodes(bs, "out1_trans1_refl2", sample, "input_1", Length::zero())?;
    scenery.connect_nodes(sample, "output_1", m1, "input_1", Length::zero())?;
    scenery.connect_nodes(m1, "output_1", r_sample, "input_1", Length::zero())?;
    scenery.connect_nodes(r_sample, "output_1", r_bs, "input_1", Length::zero())?;
    scenery.connect_nodes(bs, "out2_trans2_refl1", m2, "input_1", Length::zero())?;
    scenery.connect_nodes(m2, "output_1", r_bs, "input_2", Length::zero())?;
    scenery.connect_nodes(r_bs, "out1_trans1_refl2", det, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::Energy);
    doc.save_to_file(Path::new("./opossum/playground/michaelson.opm"))
}
