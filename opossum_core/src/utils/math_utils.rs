//! various simple helper functions (e.g. number format conversion)

use nalgebra::{Point2, Point3};
use num_traits::AsPrimitive;
use uom::si::f64::Length;

/// Converts any integer primitive to an `f64`.
///
/// This generic function centralizes the `as` cast to avoid repeating
/// `#[allow(clippy::cast_precision_loss)]` for each integer type.
///
/// # Note on Precision
/// This cast may lose precision if the source integer type has more than 53 bits
/// of precision (e.g., a `u64` or `i64` with a large value).
#[must_use]
#[inline]
#[allow(clippy::cast_precision_loss)]
pub fn to_f64<T>(value: T) -> f64
where
    T: AsPrimitive<f64>,
{
    value.as_()
}

/// Safely converts an `f64` value to a `usize`.
///
/// This function returns `Some(value)` if the `f64` is non-negative, finite, and
/// fits within the bounds of `usize`. Otherwise, it returns `None`.
/// This avoids unexpected truncation or panics from a direct `as` cast.
#[must_use]
#[inline]
pub fn try_f64_to_usize(value: f64) -> Option<usize> {
    #[allow(clippy::cast_precision_loss)]
    if value.is_sign_positive() && value.is_finite() && value <= usize::MAX as f64 {
        // This cast is now safe due to the checks above.
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        Some(value as usize)
    } else {
        None
    }
}

/// Safely converts an `f64` value to a `u8`.
///
/// This function returns `Some(value)` if the `f64` is non-negative, finite, and
/// fits within the bounds of `usize`. Otherwise, it returns `None`.
/// This avoids unexpected truncation or panics from a direct `as` cast.
#[must_use]
#[inline]
pub fn try_f64_to_u8(value: f64) -> Option<u8> {
    #[allow(clippy::cast_precision_loss)]
    if value.is_sign_positive() && value.is_finite() && value <= f64::from(u8::MAX) {
        // This cast is now safe due to the checks above.
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        Some(value as u8)
    } else {
        None
    }
}

/// Calculates the 2D Euclidean distance between two nalgebra points.
#[must_use]
pub fn distance_2d_point(point1: &Point2<Length>, point2: &Point2<Length>) -> Length {
    let dx = point1.x - point2.x;
    let dy = point1.y - point2.y;
    (dx * dx + dy * dy).sqrt()
}

/// Calculates the 3D Euclidean distance between two nalgebra points.
#[must_use]
pub fn distance_3d_point(point1: &Point3<Length>, point2: &Point3<Length>) -> Length {
    let dx = point1.x - point2.x;
    let dy = point1.y - point2.y;
    let dz = point1.z - point2.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[cfg(test)]
mod test {
    use super::*; // Use super to import from the parent module
    use crate::millimeter;
    use approx::assert_abs_diff_eq; // Assuming millimeter! is in crate root

    #[test]
    fn test_to_f64() {
        assert_eq!(to_f64(10_usize), 10.0_f64);
        assert_eq!(to_f64(-20_i32), -20.0_f64);
        assert_eq!(to_f64(30_isize), 30.0_f64);
    }

    #[test]
    fn test_try_f64_to_usize() {
        assert_eq!(try_f64_to_usize(123.45), Some(123));
        assert_eq!(try_f64_to_usize(0.0), Some(0));
        assert_eq!(try_f64_to_usize(-10.0), None); // Negative
        assert_eq!(try_f64_to_usize(f64::NAN), None); // Not a number
        assert_eq!(try_f64_to_usize(f64::INFINITY), None); // Infinite
        // Test boundary
        assert_eq!(try_f64_to_usize(usize::MAX as f64), Some(usize::MAX));
    }

    #[test]
    fn distance() {
        let p1 = millimeter!(0.0, 0.0);
        assert_eq!(
            distance_2d_point(&p1, &millimeter!(0.0, 0.0)),
            millimeter!(0.0)
        );
        assert_eq!(
            distance_2d_point(&p1, &millimeter!(1.0, 0.0)),
            millimeter!(1.0)
        );
        assert_eq!(
            distance_2d_point(&p1, &millimeter!(-1.0, 0.0)),
            millimeter!(1.0)
        );
        assert_abs_diff_eq!(
            distance_2d_point(&p1, &millimeter!(1.0, 1.0)).value,
            millimeter!(f64::sqrt(2.0)).value
        );
    }

    #[test]
    fn distance_3d() {
        let p1 = millimeter!(0.0, 0.0, 0.0);
        assert_eq!(
            distance_3d_point(&p1, &millimeter!(0.0, 0.0, 0.0)),
            millimeter!(0.0)
        );
        assert_eq!(
            distance_3d_point(&p1, &millimeter!(0.0, 0.0, 1.0)),
            millimeter!(1.0)
        );
        assert_eq!(
            distance_3d_point(&p1, &millimeter!(0.0, 0.0, -1.0)),
            millimeter!(1.0)
        );
        assert_abs_diff_eq!(
            distance_3d_point(&p1, &millimeter!(1.0, 1.0, 1.0)).value,
            millimeter!(f64::sqrt(3.0)).value
        );
    }
}
