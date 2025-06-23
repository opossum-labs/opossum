//! various simple helper functions (e.g. number format conversion)

use nalgebra::Point2;
use uom::si::f64::Length;

/// Convert a `usize` value to a `f64`.
///
/// This function is used to avoid linter warnings (precision loss) which
/// would need to be supressed each time otherwise.
///
/// **Note**: This function might potentially lead to a precision loss since a `usize` usually has
/// intenally a higher bit depth than an `f64`
#[must_use]
#[inline]
pub const fn usize_to_f64(value: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let newval = value as f64;
    newval
}

#[must_use]
#[inline]
pub const fn isize_to_f64(value: isize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let newval = value as f64;
    newval
}
/// Convert a `f64` value to a `usize`.
///
/// This function is used to avoid linter warnings (truncation, sign loss) which
/// would need to be supressed each time otherwise.
///
/// **Note**: This function might possibly lead to a truncation since the maximum value of
/// a `f64` is larger than an `usize`. Furthermore, there is no check for negative `f64` values
/// which would lead to conversion errors.
#[must_use]
#[inline]
pub const fn f64_to_usize(value: f64) -> usize {
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    let newval = value as usize;
    newval
}
#[must_use]
pub fn distance_2d_point(point1: &Point2<Length>, point2: &Point2<Length>) -> Length {
    ((point1.x - point2.x) * (point1.x - point2.x) + (point1.y - point2.y) * (point1.y - point2.y))
        .sqrt()
}

#[cfg(test)]
mod test {
    use approx::assert_abs_diff_eq;

    use crate::{millimeter, utils::math_utils::distance_2d_point};

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
}
