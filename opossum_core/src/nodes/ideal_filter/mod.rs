#![warn(missing_docs)]
//! ideal filter node
mod filter_types;
mod node;

pub use filter_types::{
    BandFilter, BandFilterType, EdgeFilter, EdgeFilterType, FilterType, FilterTypeBuilder,
    SpectralFilterBuilder,
};
pub use node::IdealFilter;
