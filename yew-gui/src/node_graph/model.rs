#[cfg(not(target_arch = "wasm32"))]
use opossum::nodes::NodeGroup;
use opossum::optic_ref::OpticRef;


pub struct OPMGUIModel{
    model: NodeGroup
}

impl OPMGUIModel{
    pub fn new(name: &str) -> Self{
        Self{model: NodeGroup::new(name)}
    }

    pub fn add_node(&mut self, node: &OpticRef){
        // let test = self.model.graph().g.add_node(node);
        // .add_node(node);
    }
}