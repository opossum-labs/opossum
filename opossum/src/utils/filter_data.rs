//!This module should contain all the functions that are used for filtering arrays, vectors and such.

use approx::{relative_eq, RelativeEq};
use num::Float;

/// This method filters out all NaN and infinite values  
/// # Attributes
/// - `ax_vals`: dynamically sized vector slice of the data vector on this axis
/// # Returns
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
/// # Attributes
/// - `ax_vals`: dynamically sized vector slice of the data vector on this axis
/// # Returns
/// If Successful, this method returns an Option containting the minimum and maximum value: Option<(min, max)>.
/// If `ax_vals` contains only non-finite values (inf, -inf, NaN), None is returned
#[must_use]
pub fn get_min_max_filter_nonfinite(ax_vals: &[f64]) -> Option<(f64, f64)> {
    let (min, max) = ax_vals
        .iter()
        .filter(|x| x.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |arg, v| {
            (f64::min(arg.0, *v), f64::max(arg.1, *v))
        });

    if !min.is_finite() || !max.is_finite() {
        None
    } else {
        Some((min, max))
    }
}
/// This method returns a vector of all unique values in an array ignoring non finite values
/// Here, unique means that the values only differ by a marginal amount according to the `relative_eq`! macro from the approx crate
/// # Attributes
/// - `array`: array of values
#[must_use]
pub fn get_unique_finite_values<T: Clone + RelativeEq + Float>(array: &[T]) -> Vec<T> {
    let mut unique_vals = Vec::<T>::new();
    let filtered_array = filter_nan_infinite(array);
    for val in &filtered_array {
        let mut unique = true;
        for unique_val in &unique_vals {
            if relative_eq!(unique_val, val) {
                unique = false;
                break;
            };
        }
        if unique {
            unique_vals.push(*val);
        }
    }
    unique_vals
}

#[cfg(test)]
mod test {
    use approx::assert_relative_eq;

    use super::*;
    #[test]
    fn get_unique_values_test() {
        let v = vec![0., 1., 10.];
        let unique = get_unique_finite_values(v.as_slice());
        assert_relative_eq!(unique[0], 0.);
        assert_relative_eq!(unique[1], 1.);
        assert_relative_eq!(unique[2], 10.);
        assert_eq!(unique.len(), 3);

        let v = vec![0., 1., 10., 1.];
        let unique = get_unique_finite_values(v.as_slice());
        assert_relative_eq!(unique[0], 0.);
        assert_relative_eq!(unique[1], 1.);
        assert_relative_eq!(unique[2], 10.);
        assert_eq!(unique.len(), 3);

        let v = vec![0., 0., 0., 0.];
        let unique = get_unique_finite_values(v.as_slice());
        assert_relative_eq!(unique[0], 0.);
        assert_eq!(unique.len(), 1);

        let v = vec![-10., 0., 0., 0.];
        let unique = get_unique_finite_values(v.as_slice());
        assert_relative_eq!(unique[0], -10.);
        assert_relative_eq!(unique[1], 0.);
        assert_eq!(unique.len(), 2);

        let v = vec![10., 0., -1., f64::NEG_INFINITY, f64::INFINITY, f64::NAN];
        let unique = get_unique_finite_values(v.as_slice());
        assert_relative_eq!(unique[0], 10.);
        assert_relative_eq!(unique[1], 0.);
        assert_relative_eq!(unique[2], -1.);
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
