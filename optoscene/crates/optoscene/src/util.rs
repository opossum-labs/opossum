//! Small crate-internal conversion helpers.

/// Casts an `f64` to the `f32` glTF/protocol storage type.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub const fn to_f32(value: f64) -> f32 {
    value as f32
}
