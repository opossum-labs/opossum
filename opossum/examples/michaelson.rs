use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    error::OpmResult,
    lightdata::{DataEnergy, LightData},
    nodes::{BeamSplitter, Detector, Dummy, NodeReference, Source},
    spectrum_helper::create_he_ne_spec,
    OpmDocument, OpticScenery,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    scenery.set_description("Michaelson interferomater");
    let src = scenery.add_node(Source::new(
        "Source",
        &LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0)?,
        }),
    ));
    let bs = scenery.add_node(BeamSplitter::default());
    let sample = scenery.add_node(Dummy::new("Sample"));
    let rf = NodeReference::from_node(&scenery.node(sample)?);
    let r_sample = scenery.add_node(rf);
    let m1 = scenery.add_node(Dummy::new("Mirror"));
    let m2 = scenery.add_node(Dummy::new("Mirror"));
    let rf = NodeReference::from_node(&scenery.node(bs)?);
    let r_bs = scenery.add_node(rf);
    let det = scenery.add_node(Detector::default());

    scenery.connect_nodes(src, "out1", bs, "input1", Length::zero())?;
    scenery.connect_nodes(bs, "out1_trans1_refl2", sample, "front", Length::zero())?;
    scenery.connect_nodes(sample, "rear", m1, "front", Length::zero())?;
    scenery.connect_nodes(m1, "rear", r_sample, "front", Length::zero())?;
    scenery.connect_nodes(r_sample, "rear", r_bs, "input1", Length::zero())?;
    scenery.connect_nodes(bs, "out2_trans2_refl1", m2, "front", Length::zero())?;
    scenery.connect_nodes(m2, "rear", r_bs, "input2", Length::zero())?;
    scenery.connect_nodes(r_bs, "out1_trans1_refl2", det, "in1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/michaelson.opm"))
}
