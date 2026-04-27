use crate::error::OpmResult;
use crate::generic_validators::*;
use crate::{millimeter, validated, validated_type, validated_vec, validated_vec_type};
use nalgebra::Point2;
use uom::si::f64::Length;

/// A validated pair of side lengths represented as a 2D point.
///
/// Both components must be:
/// - finite (not NaN or infinite)
/// - strictly positive
///
/// Typically used to represent width and height in a 2D space.
pub type ValidatedSideLengths = validated_type!(Point2<Length>, AllNormal && AllPositive);

impl ValidatedSideLengths {
    /// Attempts to create a new [`ValidatedSideLengths`] instance.
    ///
    /// # Errors
    /// Returns an error if any component is non-finite or not strictly positive.
    pub fn try_new(side_lengths: Point2<Length>) -> OpmResult<Self> {
        validated!(side_lengths, AllNormal && AllPositive)
    }
}

impl Default for ValidatedSideLengths {
    /// Returns a default value of 25 mm × 25 mm.
    ///
    /// This is guaranteed to be valid.
    fn default() -> Self {
        Self::try_new(millimeter!(25.0, 25.0)).unwrap()
    }
}

/// A validated 2D center point.
///
/// Both coordinates must be finite (not NaN or infinite).
pub type ValidatedCenter = validated_type!(Point2<Length>, AllFinite);

impl ValidatedCenter {
    /// Attempts to create a new [`ValidatedCenter`] instance.
    ///
    /// # Errors
    /// Returns an error if any coordinate is NaN or infinite.
    pub fn try_new(center: Point2<Length>) -> OpmResult<Self> {
        validated!(center, AllFinite)
    }
}

impl Default for ValidatedCenter {
    /// Returns the origin (0 mm, 0 mm).
    ///
    /// This is guaranteed to be valid.
    fn default() -> Self {
        Self::try_new(millimeter!(0.0, 0.0)).unwrap()
    }
}

/// A validated radius value.
///
/// The value must be:
/// - finite (not NaN or infinite)
/// - strictly positive
pub type ValidatedRadius = validated_type!(Length, AllPositive && AllFinite);

impl ValidatedRadius {
    /// Attempts to create a new [`ValidatedRadius`] instance.
    ///
    /// # Errors
    /// Returns an error if the radius is not finite or not strictly positive.
    pub fn try_new(radius: Length) -> OpmResult<Self> {
        validated!(radius, AllPositive && AllFinite)
    }
}

impl Default for ValidatedRadius {
    /// Returns a default radius of 25 mm.
    ///
    /// This is guaranteed to be valid.
    fn default() -> Self {
        Self::try_new(millimeter!(25.0)).unwrap()
    }
}

pub type ValidatedPolygonPoints =
    validated_vec_type!(Vec<(Point2<Length>)>, AllFinite && AllNotNan, Min3Entries);

impl ValidatedPolygonPoints {
    pub fn try_new(points: Vec<Point2<Length>>) -> OpmResult<Self> {
        validated_vec!(points, AllFinite && AllNotNan, Min3Entries)
    }
}

impl Default for ValidatedPolygonPoints {
    fn default() -> Self {
        Self::try_new(vec![
            Point2::new(millimeter!(-12.5), millimeter!(-12.5)),
            Point2::new(millimeter!(12.5), millimeter!(-12.5)),
            Point2::new(millimeter!(12.5), millimeter!(12.5)),
            Point2::new(millimeter!(-12.5), millimeter!(12.5)),
        ])
        .unwrap()
    }
}
