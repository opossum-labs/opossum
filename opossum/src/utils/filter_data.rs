//! This module should contain all the functions that are used for filtering arrays, vectors and such.

use approx::{RelativeEq, relative_eq};
use num::{Float, Num};

/// This method filters out all NaN and infinite values
///  
/// # Attributes
///
/// - `ax_vals`: dynamically sized vector slice of the data vector on this axis
///
/// # Returns
///
/// This method returns an array containing only the non-NaN and finite entries of the passed vector
#[must_use]
pub fn filter_nan_infinite<T: Float>(ax_vals: &[T]) -> Vec<T> {
    ax_vals
        .iter()
        .copied()
        .filter(|x| x.is_finite())
        .collect::<Vec<T>>()
}

/// This method returns the minimum and maximum value of the provided values while ignoring non-finite values
///
/// # Attributes
///
/// - `ax_vals`: dynamically sized vector slice of the data vector on this axis
///
/// # Returns
///
/// If Successful, this method returns an Option containting the minimum and maximum value: Option<(min, max)>.
/// If `ax_vals` contains only non-finite values (inf, -inf, NaN), None is returned
#[must_use]
pub fn get_min_max_filter_nonfinite<T: Float>(ax_vals: &[T]) -> Option<(T, T)> {
    let (min, max) = ax_vals.iter().copied().filter(|x| x.is_finite()).fold(
        (T::infinity(), T::neg_infinity()),
        |(current_min, current_max), val| (current_min.min(val), current_max.max(val)),
    );
    if !min.is_finite() || !max.is_finite() {
        None
    } else {
        Some((min, max))
    }
}
#[must_use]
/// Gets all unique values in an array
///
/// This method returns a vector of all unique values in an array ignoring non finite values. Note that the resulting
/// array is sorted in ascending order. Hence, the original order is not retained.
/// Here, unique means that the values only differ by a marginal amount according to the `relative_eq`! macro from the approx crate
///
/// # Attributes
///
/// - `array`: array of values
///
/// # Panics
///
/// This function might only theoretically panic.
pub fn get_unique_finite_values_sorted<T: Clone + RelativeEq + Num + Float>(array: &[T]) -> Vec<T> {
    let mut filtered_array = filter_nan_infinite(array);

    if filtered_array.is_empty() {
        return Vec::new();
    }
    // NaNs are filtered, so unwrap is safe.
    filtered_array.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());

    let mut unique_vals = Vec::new();
    unique_vals.push(filtered_array[0]); // Add the first element

    for value in filtered_array.iter().skip(1) {
        // Compare with the last added unique value.
        // Need to dereference unique_vals.last().unwrap() if T is not Copy,
        // but Float implies Copy for f32/f64.
        if !relative_eq!(*value, *unique_vals.last().unwrap()) {
            unique_vals.push(*value);
        }
    }
    unique_vals
}
#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_relative_eq;
    #[test]
    fn get_unique_values_sorted_test() {
        let v = vec![0., 1., 10.];
        let unique = get_unique_finite_values_sorted(v.as_slice());
        assert_relative_eq!(unique[0], 0.);
        assert_relative_eq!(unique[1], 1.);
        assert_relative_eq!(unique[2], 10.);
        assert_eq!(unique.len(), 3);

        let v = vec![0., 1., 10., 1.];
        let unique = get_unique_finite_values_sorted(v.as_slice());
        assert_relative_eq!(unique[0], 0.);
        assert_relative_eq!(unique[1], 1.);
        assert_relative_eq!(unique[2], 10.);
        assert_eq!(unique.len(), 3);

        let v = vec![0., 0., 0., 0.];
        let unique = get_unique_finite_values_sorted(v.as_slice());
        assert_relative_eq!(unique[0], 0.);
        assert_eq!(unique.len(), 1);

        let v = vec![-10., 0., 0., 0.];
        let unique = get_unique_finite_values_sorted(v.as_slice());
        assert_relative_eq!(unique[0], -10.);
        assert_relative_eq!(unique[1], 0.);
        assert_eq!(unique.len(), 2);

        let v = vec![10., 0., -1., f64::NEG_INFINITY, f64::INFINITY, f64::NAN];
        let unique = get_unique_finite_values_sorted(v.as_slice());
        assert_relative_eq!(unique[0], -1.);
        assert_relative_eq!(unique[1], 0.);
        assert_relative_eq!(unique[2], 10.);
        assert_eq!(unique.len(), 3);
    }
    #[test]
    fn filter_nan_infinite_test() {
        let v = vec![0., 1., 10.];
        let v_filter = filter_nan_infinite(v.as_slice());
        assert_relative_eq!(v_filter[0], 0.);
        assert_relative_eq!(v_filter[1], 1.);
        assert_relative_eq!(v_filter[2], 10.);
        assert_eq!(v_filter.len(), 3);

        let v = vec![-0., -1., -10.];
        let v_filter = filter_nan_infinite(v.as_slice());
        assert_relative_eq!(v_filter[0], -0.);
        assert_relative_eq!(v_filter[1], -1.);
        assert_relative_eq!(v_filter[2], -10.);
        assert_eq!(v_filter.len(), 3);

        let v = vec![0., 1., 10., f64::NEG_INFINITY, f64::INFINITY, f64::NAN];
        let v_filter = filter_nan_infinite(v.as_slice());
        assert_relative_eq!(v_filter[0], 0.);
        assert_relative_eq!(v_filter[1], 1.);
        assert_relative_eq!(v_filter[2], 10.);
        assert_eq!(v_filter.len(), 3);

        let v = vec![f64::NEG_INFINITY, f64::INFINITY, f64::NAN, 0., 1., 10.];
        let v_filter = filter_nan_infinite(v.as_slice());
        assert_relative_eq!(v_filter[0], 0.);
        assert_relative_eq!(v_filter[1], 1.);
        assert_relative_eq!(v_filter[2], 10.);
        assert_eq!(v_filter.len(), 3);

        let v = vec![0., f64::NEG_INFINITY, f64::INFINITY, f64::NAN];
        let v_filter = filter_nan_infinite(v.as_slice());
        assert_relative_eq!(v_filter[0], 0.);
        assert_eq!(v_filter.len(), 1);

        let v = vec![f64::NEG_INFINITY, f64::INFINITY, f64::NAN];
        let v_filter = filter_nan_infinite(v.as_slice());
        assert!(v_filter.is_empty());
    }
    #[test]
    fn get_min_max_filter_nonfinite_test() {
        let v = vec![0., 1., 10.];
        let (min, max) = get_min_max_filter_nonfinite(v.as_slice()).unwrap();
        assert_relative_eq!(min, 0.);
        assert_relative_eq!(max, 10.);

        let v = vec![-0., -1., -10.];
        let (min, max) = get_min_max_filter_nonfinite(v.as_slice()).unwrap();
        assert_relative_eq!(max, 0.);
        assert_relative_eq!(min, -10.);

        let v = vec![0., 1., 10., f64::NEG_INFINITY, f64::INFINITY, f64::NAN];
        let (min, max) = get_min_max_filter_nonfinite(v.as_slice()).unwrap();
        assert_relative_eq!(min, 0.);
        assert_relative_eq!(max, 10.);

        let v = vec![0., f64::NEG_INFINITY, f64::INFINITY, f64::NAN];
        let (min, max) = get_min_max_filter_nonfinite(v.as_slice()).unwrap();
        assert_relative_eq!(min, 0.);
        assert_relative_eq!(max, 0.);

        let v = vec![f64::NEG_INFINITY, f64::INFINITY, f64::NAN];
        assert_eq!(get_min_max_filter_nonfinite(v.as_slice()), None);
    }
}
