use nalgebra::{Isometry2, Point2, Vector2};
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::meter};

use super::{ApertureType, Apodize};
use crate::error::{OpmResult, OpossumError};

/// Configuration data for a rectangular aperture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RectangleConfig {
    width: Length,
    height: Length,
    center: Point2<Length>,
    aperture_type: ApertureType,
}
impl RectangleConfig {
    /// Create a new rectangular aperture configuration by given width, height and the center point.
    ///
    /// By default the aperture has the aperture type [`ApertureType::Hole`].
    /// # Errors
    ///
    /// This function will return an error if width and/or height are negative, NaN or Infinity.
    pub fn new(width: Length, height: Length, center: Point2<Length>) -> OpmResult<Self> {
        if width.is_normal()
            && width.is_sign_positive()
            && height.is_normal()
            && height.is_sign_positive()
            && center.coords[0].is_finite()
            && center.coords[1].is_finite()
        {
            Ok(Self {
                width,
                height,
                center,
                aperture_type: ApertureType::default(),
            })
        } else {
            Err(OpossumError::Other(
                "height & width must be positive".into(),
            ))
        }
    }

    /// Returns the width of this [`RectangleConfig`].
    #[must_use]
    pub fn width(&self) -> Length {
        self.width
    }
    /// Returns the height of this [`RectangleConfig`].
    #[must_use]
    pub fn height(&self) -> Length {
        self.height
    }
    /// Returns the center of this [`RectangleConfig`].
    #[must_use]
    pub fn center(&self) -> Point2<Length> {
        self.center
    }
}
impl Apodize for RectangleConfig {
    fn set_aperture_type(&mut self, aperture_type: ApertureType) {
        self.aperture_type = aperture_type;
    }
    fn apodize(&self, point: &Point2<Length>) -> f64 {
        let translation = Isometry2::translation(
            self.center.coords[0].get::<meter>(),
            self.center.coords[1].get::<meter>(),
        );
        let point_meter = Point2::<f64>::new(point.x.get::<meter>(), point.y.get::<meter>());
        let point_transformed = translation.inverse_transform_point(&point_meter);

        let q = Vector2::new(
            point_transformed.x.abs() - self.width.get::<meter>() / 2.,
            point_transformed.y.abs() - self.height.get::<meter>() / 2.,
        );
        let mut q_max = q;
        q_max.iter_mut().for_each(|x: &mut f64| *x = x.max(0.0));
        let sdf_val = q_max.x.mul_add(q_max.x, q_max.y.powi(2)).sqrt() + q.x.max(q.y).min(0.0);

        let mut transmission = if sdf_val <= 0. { 1.0 } else { 0.0 };
        if matches!(self.aperture_type, ApertureType::Obstruction) {
            transmission = 1.0 - transmission;
        }
        transmission
    }
}
