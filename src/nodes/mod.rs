//! This module contains the concrete node types (lenses, filters, etc...)
mod node_dummy;
mod node_reference;
mod node_group;
mod node_beam_splitter;

pub use node_dummy::NodeDummy;
pub use node_reference::NodeReference;
pub use node_group::NodeGroup;
pub use node_beam_splitter::NodeBeamSplitter;