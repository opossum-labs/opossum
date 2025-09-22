use nalgebra::{Isometry2, Point2, Vector2};
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::meter};

use super::Shape;
use crate::error::{OpmResult, OpossumError};

/// Configuration data for a rectangular aperture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RectangleShape {
    width: Length,
    height: Length,
    center: Point2<Length>,
}
impl RectangleShape {
    /// Create a new rectangular aperture configuration by given width, height and the center point.
    ///
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
            })
        } else {
            Err(OpossumError::Other(
                "height & width must be positive".into(),
            ))
        }
    }

    /// Returns the width of this [`RectangleShape`].
    #[must_use]
    pub fn width(&self) -> Length {
        self.width
    }
    /// Returns the height of this [`RectangleShape`].
    #[must_use]
    pub fn height(&self) -> Length {
        self.height
    }
    /// Returns the center of this [`RectangleShape`].
    #[must_use]
    pub fn center(&self) -> Point2<Length> {
        self.center
    }
}
impl Shape for RectangleShape {
    fn transmission_factor(&self, point: &Point2<Length>) -> f64 {
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

        if sdf_val <= 0. { 1.0 } else { 0.0 }
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::meter;
    #[test]
    fn new() {
        let p = meter!(0.0, 0.0);
        assert!(RectangleShape::new(meter!(2.0), meter!(1.0), p).is_ok());
        assert!(RectangleShape::new(meter!(0.0), meter!(1.0), p).is_err());
        assert!(RectangleShape::new(meter!(-1.0), meter!(1.0), p).is_err());
        assert!(RectangleShape::new(meter!(f64::NAN), meter!(1.0), p).is_err());
        assert!(RectangleShape::new(meter!(f64::INFINITY), meter!(1.0), p).is_err());
        assert!(RectangleShape::new(meter!(1.0), meter!(0.0), p).is_err());
        assert!(RectangleShape::new(meter!(1.0), meter!(-1.0), p).is_err());
        assert!(RectangleShape::new(meter!(1.0), meter!(f64::NAN), p).is_err());
        assert!(RectangleShape::new(meter!(1.0), meter!(f64::INFINITY), p).is_err());
        let p = meter!(f64::NAN, 0.0);
        assert!(RectangleShape::new(meter!(2.0), meter!(1.0), p).is_err());
        let p = meter!(f64::INFINITY, 0.0);
        assert!(RectangleShape::new(meter!(2.0), meter!(1.0), p).is_err());
    }
    #[test]
    fn transmission_factor() {
        let r = RectangleShape::new(meter!(1.0), meter!(2.0), meter!(1.0, 1.0)).unwrap();
        assert_eq!(r.transmission_factor(&meter!(1.0, 1.0)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(1.5, 1.0)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(1.5, 2.0)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(0.5, 2.0)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(0.5, 0.0)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(0.0, 0.0)), 0.0);
        assert_eq!(r.transmission_factor(&meter!(1.0, 2.1)), 0.0);
    }
}
