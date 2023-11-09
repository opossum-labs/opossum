use opossum::error::OpossumError;
use opossum::nodes::{Dummy, NodeReference};
use opossum::optical::Optical;
use opossum::OpticScenery;
use std::path::Path;

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("PreAmp Doublepass section");
    //let n0 = scenery.add_element("LightSource", Source::default());
    let n1 = scenery.add_node(Dummy::new("TFP"));
    let n2 = scenery.add_node(Dummy::new("19mm amp"));
    //let n3 = scenery.add_element("Faraday", Dummy);
    let n4 = scenery.add_node(Dummy::new("0° mirror"));

    let mut node = NodeReference::from_node(scenery.node(n1).unwrap());
    node.set_property("inverted", true.into()).unwrap();
    let n1r = scenery.add_node(node);

    // let mut node= NodeReference::new(scenery.node(n3).unwrap());
    // node.set_inverted(true);
    // let n3r = scenery.add_node(node);

    let mut node = NodeReference::from_node(scenery.node(n2)?);
    node.set_property("inverted", true.into()).unwrap();
    let n2r = scenery.add_node(node);

    // scenery.connect_nodes(n0, "out1", n1, "front").unwrap();
    scenery.connect_nodes(n1, "rear", n2, "front")?;
    scenery.connect_nodes(n2, "rear", n4, "front")?;
    //  scenery.connect_nodes(n3, "rear", n4, "front").unwrap();
    scenery.connect_nodes(n4, "rear", n2r, "rear")?;
    // scenery.connect_nodes(n3r, "front", n2r, "rear").unwrap();
    scenery.connect_nodes(n2r, "front", n1r, "rear")?;

    // let mut group = NodeGroup::new();
    // let g_n1 = group.add_node(OpticNode::new("Beamsplitter", Dummy));
    // let g_n2 = group.add_node(OpticNode::new("Lens", Dummy));
    // let g_n3 = group.add_node(OpticNode::new("Lens2", Dummy));
    // let g_n4  = group.add_node(OpticNode::new("Det", Detector::default()));

    // group.connect_nodes(g_n1, "rear", g_n2, "front").unwrap();
    // group.connect_nodes(g_n2, "rear", g_n3, "front").unwrap();
    // group.connect_nodes(g_n3, "rear", g_n4, "in1").unwrap();
    // scenery.add_node(OpticNode::new("CamBox", group));
    scenery.save_to_file(Path::new("playground/pa_doublepass.opm"))?;
    Ok(())
}
