// use uuid::Uuid;

// #[cfg(not(target_arch = "wasm32"))]
// use opossum;
// #[cfg(not(target_arch = "wasm32"))]
// use opossum::nodes::NodeGroup;

// // #[cfg(not(target_arch = "wasm32"))]
// use opossum::{error::OpmResult, optic_ref::OpticRef};

// #[derive(Clone)]
// pub struct OPMGUIModel{
//     model: NodeGroup
// }

// impl OPMGUIModel{
//     pub fn new(name: &str) -> Self{
//         Self{model: NodeGroup::new(name)}
//     }

//     pub fn add_node(&mut self, node: &OpticRef) -> OpmResult<Uuid>{
//         self.model.add_node_ref(node)?;
//         Ok(node.uuid())
//     }
// }