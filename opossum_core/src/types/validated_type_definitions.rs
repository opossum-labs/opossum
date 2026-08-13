use crate::error::OpmResult;
use crate::{
    apertures::{Aperture, GeometricBound},
    degree,
    generic_validators::{AllFinite, AllNormal, AllNotNan, AllPositive, Min3Entries},
};
use crate::{millimeter, validated, validated_type, validated_vec, validated_vec_type};
use nalgebra::Point2;
use uom::si::f64::{Angle, Length};

/// A validated pair of side lengths represented as a 2D point.
///
/// Both components must be:
/// - finite (not NaN or infinite)
/// - strictly positive
///
/// Typically used to represent width and height in a 2D space.
pub type ValidatedSideLengths2D = validated_type!(Point2<Length>, AllNormal && AllPositive);

impl ValidatedSideLengths2D {
    /// Attempts to create a new [`ValidatedSideLengths2D`] instance.
    ///
    /// # Errors
    /// Returns an error if any component is non-finite or not strictly positive.
    pub fn try_new(side_lengths: Point2<Length>) -> OpmResult<Self> {
        validated!(side_lengths, AllNormal && AllPositive)
    }
}

impl Default for ValidatedSideLengths2D {
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
pub type ValidatedCenter2D = validated_type!(Point2<Length>, AllFinite);

impl ValidatedCenter2D {
    /// Attempts to create a new [`ValidatedCenter2D`] instance.
    ///
    /// # Errors
    /// Returns an error if any coordinate is NaN or infinite.
    pub fn try_new(center: Point2<Length>) -> OpmResult<Self> {
        validated!(center, AllFinite)
    }
}

impl Default for ValidatedCenter2D {
    /// Returns the origin (0 mm, 0 mm).
    ///
    /// This is guaranteed to be valid.
    fn default() -> Self {
        Self::try_new(millimeter!(0.0, 0.0)).unwrap()
    }
}

/// A validated 2D angle value.
///
/// Rotation must be finite (not NaN or infinite).
pub type ValidatedAngle1D = validated_type!(Angle, AllFinite);

impl ValidatedAngle1D {
    /// Attempts to create a new [`ValidatedAngle1D`] instance.
    ///
    /// # Errors
    /// Returns an error if angle is NaN or infinite.
    pub fn try_new(angle: Angle) -> OpmResult<Self> {
        validated!(angle, AllFinite)
    }
}

impl Default for ValidatedAngle1D {
    /// Returns a zero rotation.
    ///
    /// This is guaranteed to be valid.
    fn default() -> Self {
        Self::try_new(degree!(0.0)).unwrap()
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

/// A validated transversal boundary of a body.
///
/// The [`Aperture`] must delimit a region, i.e. be a binary shape acting as a hole — see
/// [`Aperture::is_geometric_bound`]. Storing the boundary in this type is what keeps a body from
/// being handed a transmission mask instead of an outline: a soft-edged, inverted or open aperture
/// leaves it undefined where the medium ends, so it is rejected on construction rather than
/// silently misread later on.
pub type ValidatedCrossSection = validated_type!(Aperture, GeometricBound);

impl ValidatedCrossSection {
    /// Attempts to create a new [`ValidatedCrossSection`] instance.
    ///
    /// # Errors
    /// Returns an error if the aperture does not delimit a region.
    pub fn try_new(aperture: Aperture) -> OpmResult<Self> {
        validated!(aperture, GeometricBound)
    }
}

pub type ValidatedPolygonPoints2D =
    validated_vec_type!(Vec<Point2<Length>>, AllFinite && AllNotNan, Min3Entries);

impl ValidatedPolygonPoints2D {
    /// Attempts to create a new [`ValidatedPolygonPoints2D`] instance.
    /// # Errors
    /// Returns an error if any point is non-finite, contains NaN, or if there are fewer than 3 points.
    pub fn try_new(points: Vec<Point2<Length>>) -> OpmResult<Self> {
        validated_vec!(points, AllFinite && AllNotNan, Min3Entries)
    }
}

impl Default for ValidatedPolygonPoints2D {
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
