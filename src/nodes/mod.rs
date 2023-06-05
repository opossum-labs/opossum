//! This module contains the concrete node types (lenses, filters, etc...)
mod node_dummy;
mod node_reference;

pub use node_dummy::NodeDummy;
pub use node_reference::NodeReference;