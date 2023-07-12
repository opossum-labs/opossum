use opossum::{
    nodes::{BeamSplitter, Dummy, NodeReference, Source, Detector},
    optic_scenery::OpticScenery,
};
use std::fs::File;
use std::io::Write;

fn main() {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Michaelson interferomater".into());
    let src = scenery.add_element("Source", Source::default());
    let bs = scenery.add_element("Beamspliiter", BeamSplitter::default());
    let sample = scenery.add_element("sample", Dummy);
    let rf = NodeReference::new(scenery.node(sample).unwrap());
    let r_sample=scenery.add_node(rf);
    let m1 = scenery.add_element("Mirror", Dummy);
    let m2 = scenery.add_element("Mirror", Dummy);
    let rf = NodeReference::new(scenery.node(bs).unwrap());
    let r_bs = scenery.add_node(rf);
    let det=scenery.add_element("Detector", Detector::default());

    scenery.connect_nodes(src, "out1", bs, "input1").unwrap();
    scenery
        .connect_nodes(bs, "out1_trans1_refl2", sample, "front")
        .unwrap();
    scenery.connect_nodes(sample, "rear", m1, "front").unwrap();
    scenery.connect_nodes(m1,"rear", r_sample, "front").unwrap();
    scenery.connect_nodes(r_sample, "rear", r_bs, "input1").unwrap();
    scenery.connect_nodes(bs,"out2_trans2_refl1", m2, "front").unwrap();
    scenery.connect_nodes(m2,"rear", r_bs,"input2").unwrap();
    scenery.connect_nodes(r_bs, "out1_trans1_refl2", det, "in1").unwrap();

    let path = "michaelson.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()).unwrap();
}
