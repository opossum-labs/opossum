#![warn(missing_docs)]

mod analysis;
mod construction;
pub mod delta;
mod graph;
mod inspection;
mod serialization;
mod visualization;

pub(crate) use analysis::InvertGraphGuard;
pub use graph::{ConnectionInfo, OpticGraph};
