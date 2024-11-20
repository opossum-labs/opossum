#[must_use]
pub const fn usize_to_f64(value: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let newval = value as f64;
    newval
}

#[must_use]
pub const fn f64_to_usize(value: f64) -> usize {
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    let newval = value as usize;
    newval
}
