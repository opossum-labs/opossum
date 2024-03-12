//!This module should contain all the functions that are used for filtering arrays, vectors and such.

use approx::{relative_eq, RelativeEq};
use nalgebra::DVector;

/// This method filters out all NaN and infinite values  
/// # Attributes
/// - `ax_vals`: dynamically sized vector slice of the data vector on this axis
/// # Returns
/// This method returns an array containing only the non-NaN and finite entries of the passed vector
#[must_use]
pub fn filter_nan_infinite(ax_vals: &[f64]) -> DVector<f64> {
    DVector::from(
        ax_vals
            .iter()
            .copied()
            .filter(|x| x.is_finite())
            .collect::<Vec<f64>>(),
    )
}
/// This method returns a vector of all unique values in an array. 
/// Here, unique means that the values only differ by a marginal amount according to the relative_eq! macro from the approx crate
/// # Attributes
/// - `array`: array of values
#[must_use]
pub fn get_unique_values<T:Clone + RelativeEq>(array: &[T]) -> Vec<T> {
    let mut unique_vals = Vec::<T>::new();

    for val in array{
        let mut unique = true;
        for unique_val in &unique_vals{
            if relative_eq!(unique_val, val){
                unique = false;
                break;
            };
        }
        if unique{
            unique_vals.push(val.clone());
        }
    }
    unique_vals
}
