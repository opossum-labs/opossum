//! various simple helper functions (e.g. number format conversion)

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
