use schemars::JsonSchema;

/// Helper schema type representing a physical scalar quantity serialized as a plain `f64` in SI base units.
#[derive(JsonSchema)]
#[allow(dead_code)]
pub struct SiScalarSchema(f64);

/// Helper schema type representing a 3D vector `[x, y, z]` serialized as a 3-element array.
#[derive(JsonSchema)]
#[allow(dead_code)]
pub struct Vector3Schema([f64; 3]);

/// Helper schema type representing a 2D vector `[x, y]` serialized as a 2-element array.
#[derive(JsonSchema)]
#[allow(dead_code)]
pub struct Vector2Schema([f64; 2]);

/// Helper schema type representing a 2D point `(x, y)` serialized as a 2-element tuple.
#[derive(JsonSchema)]
#[allow(dead_code)]
pub struct Point2((f64, f64));