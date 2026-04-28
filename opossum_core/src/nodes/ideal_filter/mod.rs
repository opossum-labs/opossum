#![warn(missing_docs)]
//! ideal filter node
mod filter_types;
mod node;

use crate::error::OpmResult;
pub use filter_types::{
    BandFilter, BandFilterType, EdgeFilter, EdgeFilterType, FilterConst, FilterType,
    FilterTypeBuilder, SpectralFilterBuilder,
};
pub use node::IdealFilter;
use uom::si::f64::{Length, Ratio};

/// Trait for ideal filters that can be used in the `IdealFilter` node.
///
/// The `transmission` method returns the transmission of the filter at a given wavelength.
/// The transmission is a value between 0 and 1, where 0 means no transmission and 1 means full transmission.
pub trait EnergyFilter {
    /// The `transmission` method returns the transmission of the filter at a given wavelength.
    /// The transmission is a value between 0 and 1, where 0 means no transmission and 1 means full transmission.
    ///
    /// # Errors
    /// Returns an error if the transmission can not be derived (e.g. wavelength is out of the valid range for the filter).
    fn transmission(&self, wavelength: Length) -> OpmResult<Ratio>;
}
