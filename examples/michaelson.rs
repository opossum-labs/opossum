use opossum::{
    error::OpossumError,
    nodes::{BeamSplitter, Detector, Dummy, NodeReference, Source},
    OpticScenery,
};
use std::fs::File;
use std::io::Write;

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Michaelson interferomater");
    let src = scenery.add_node(Source::default());
    let bs = scenery.add_element("Beamspliiter", BeamSplitter::default());
    let sample = scenery.add_node(Dummy::new("Sample"));
    let rf = NodeReference::from_node(scenery.node(sample)?);
    let r_sample = scenery.add_node(rf);
    let m1 = scenery.add_node(Dummy::new("Mirror"));
    let m2 = scenery.add_node(Dummy::new("Mirror"));
    let rf = NodeReference::from_node(scenery.node(bs)?);
    let r_bs = scenery.add_node(rf);
    let det = scenery.add_element("Detector", Detector::default());

    scenery.connect_nodes(src, "out1", bs, "input1")?;
    scenery.connect_nodes(bs, "out1_trans1_refl2", sample, "front")?;
    scenery.connect_nodes(sample, "rear", m1, "front")?;
    scenery.connect_nodes(m1, "rear", r_sample, "front")?;
    scenery.connect_nodes(r_sample, "rear", r_bs, "input1")?;
    scenery.connect_nodes(bs, "out2_trans2_refl1", m2, "front")?;
    scenery.connect_nodes(m2, "rear", r_bs, "input2")?;
    scenery.connect_nodes(r_bs, "out1_trans1_refl2", det, "in1")?;

    let path = "michaelson.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()?).unwrap();
    Ok(())
}
