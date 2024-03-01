//!This module should contain all the functions that are used for filtering arrays, vectors and such.

use nalgebra::{DVector, DVectorSlice};


/// This method filters out all NaN and infinite values  
/// # Attributes
/// - `ax_vals`: dynamically sized vector slice of the data vector on this axis
/// # Returns
/// This method returns an array containing only the non-NaN and finite entries of the passed vector
#[must_use]
pub fn filter_nan_infinite(ax_vals: &DVectorSlice<'_, f64>) -> DVector<f64> {
    DVector::from(
        ax_vals
            .iter()
            .copied()
            .filter(|x| x.is_finite())
            .collect::<Vec<f64>>(),
    )
}