pub mod node_element;
pub mod model;

#[cfg(not(target_arch = "wasm32"))]
use opossum::nodes::NodeGroup;
