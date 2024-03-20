#![warn(missing_docs)]
//! Module for handling optical (2D) apertures
//!
//! An [`Aperture`] commonly defines the shape of an optical element which transmits or obstructs an incoming optical ray.
//! Currently there are "binary" shapes which either fully transmits or fully blocks a ray at a given point. Furthermore, an variable
//! transmission Gaussian aperture exists. Finally a set of apertures can be stacked on top of each other in order form aperture shapes
//! of higher complexity.
//!
//! Apertures a defined by their respective configuration struct. For the calculation the function
//! [`apodization_factor`](Aperture::apodization_factor()) is used.
//! ```rust
//! use nalgebra::Point2;
//! use opossum::aperture::{Aperture, ApertureType, CircleConfig};
//!
//! let c = CircleConfig::new(1.0, Point2::new(1.0, 1.0)).unwrap();
//! let ap = Aperture::BinaryCircle(c);
//! assert_eq!(ap.apodization_factor(&Point2::new(1.0, 1.0)), 1.0);
//! assert_eq!(ap.apodization_factor(&Point2::new(0.0, 0.0)), 0.0);
//! ```
//! Furthermore, each aperture can act as a "hole" or as an "obstruction". By default,
//! all configurations are created as "holes".
//! ```rust
//! use nalgebra::Point2;
//! use opossum::aperture::{Aperture, ApertureType, Apodize, CircleConfig};
//!
//! let mut c = CircleConfig::new(1.0, Point2::new(1.0, 1.0)).unwrap();
//! c.set_aperture_type(ApertureType::Obstruction);
//! let ap = Aperture::BinaryCircle(c);
//! assert_eq!(ap.apodization_factor(&Point2::new(1.0, 1.0)), 0.0);
//! assert_eq!(ap.apodization_factor(&Point2::new(0.0, 0.0)), 1.0);
//! ```
use crate::{
    error::{OpmResult, OpossumError},
    properties::Proptype,
};
use nalgebra::{Isometry2, Point2, Vector2};
use ncollide2d::{
    query::PointQuery,
    shape::{Ball, Cuboid, Polyline},
};
use serde_derive::{Deserialize, Serialize};
use uom::si::{f64::Length, length::meter, ratio::ratio};


macro_rules! meter {
    ($val:expr) => {
        Length::new::<meter>($val)
    };
}
/// The apodization type of an [`Aperture`].
///
/// Each aperture can act as a "hole" or "obstruction"
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub enum ApertureType {
    /// the [`Aperture`] shape acts as a hole. The inner part of the shape is transparent.
    #[default]
    Hole,
    /// the [`Aperture`] shape represents an obstruction. The inner part of the shape is opaque.
    Obstruction,
}

/// Different aperture types
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub enum Aperture {
    /// completely transparent aperture. This is the default.
    #[default]
    None,
    /// binary (either transparent or opaque) circular aperture defined by a radius and center point
    BinaryCircle(CircleConfig),
    /// binary (either transparent or opaque) rectangular aperture defined by width and height as well as its center point
    BinaryRectangle(RectangleConfig),
    /// binary (either transparent or opaque) polygonial aperture defined by a set of 2D points. This polygon can also be
    /// non-convex but should not intersect.
    BinaryPolygon(PolygonConfig),
    /// variable transmission aperture using a 2D Gaussian function.
    Gaussian(GaussianConfig),
    /// a stack of an arbitrary number of the above apertures. The transmission factor at a given point is the
    /// product of all indiviual aperture on the stack (subtractive apodization).
    Stack(StackConfig),
}
impl Aperture {
    /// Calculate the transmission factor of a given point on the [`Aperture`]. The value is in the range (0.0..=1.0)
    /// 0.0 is fully opaque, 1.0 fully transparent.
    #[must_use]
    pub fn apodization_factor(&self, point: &Point2<Length>) -> f64 {
        match self {
            Self::None => 1.0,
            Self::BinaryCircle(circle) => circle.apodize(point),
            Self::BinaryRectangle(rectangle) => rectangle.apodize(point),
            Self::BinaryPolygon(p) => p.apodize(point),
            Self::Gaussian(g) => g.apodize(point),
            Self::Stack(s) => s.apodize(point),
        }
    }
}

impl From<Aperture> for Proptype {
    fn from(value: Aperture) -> Self {
        Self::Aperture(value)
    }
}
/// A trait for all kinds of (2D-) apodizers.
pub trait Apodize {
    /// Set the apodizition type of the aperture.
    fn set_aperture_type(&mut self, aperture_type: ApertureType);

    /// Calculate the transmission coefficient for a point.
    ///
    /// This function calculates the transmission coefficient (0.0..=1.0) of an [`Aperture`] for a given 2D point.
    /// In case of a binary aperture this value is either 0.0 or 1.0 depending on whether the given point is inside
    /// or outside the aperture. For [`Aperture::Gaussian`] the function returns a continous transmission value.
    fn apodize(&self, point: &Point2<Length>) -> f64;
}
/// Configuration data for a circular aperture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleConfig {
    radius: Length,
    center: Point2<Length>,
    aperture_type: ApertureType,
}
impl CircleConfig {
    /// Create a new [`CircleConfig`] from a given radius and a center point.
    ///
    /// By default the aperture has the aperture type [`ApertureType::Hole`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the given radius of negative, NaN or Infinity.
    pub fn new(radius: Length, center: Point2<Length>) -> OpmResult<Self> {
        if radius.is_normal() && radius.is_sign_positive() {
            Ok(Self {
                radius,
                center,
                aperture_type: ApertureType::default(),
            })
        } else {
            Err(OpossumError::Other("radius must be positive".into()))
        }
    }
}
impl Apodize for CircleConfig {
    fn set_aperture_type(&mut self, aperture_type: ApertureType) {
        self.aperture_type = aperture_type;
    }
    fn apodize(&self, point: &Point2<Length>) -> f64 {
        let ball = Ball::new(self.radius.get::<meter>());
        let translation = Isometry2::translation(self.center.coords[0].get::<meter>(), self.center.coords[1].get::<meter>());
        let point_meter = Point2::<f64>::new(point.x.get::<meter>(), point.y.get::<meter>());
        let mut transmission = if ball.contains_point(&translation, &point_meter) {
            1.0
        } else {
            0.0
        };
        if matches!(self.aperture_type, ApertureType::Obstruction) {
            transmission = 1.0 - transmission;
        }
        transmission
    }
}
/// Configuration data for a rectangular aperture.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}
impl Apodize for RectangleConfig {
    fn set_aperture_type(&mut self, aperture_type: ApertureType) {
        self.aperture_type = aperture_type;
    }
    fn apodize(&self, point: &Point2<Length>) -> f64 {
        let rectangle = Cuboid {
            half_extents: Vector2::new(self.width.get::<meter>() / 2.0, self.height.get::<meter>() / 2.0),
        };
        let translation = Isometry2::translation(self.center.coords[0].get::<meter>(), self.center.coords[1].get::<meter>());
        let point_meter = Point2::<f64>::new(point.x.get::<meter>(), point.y.get::<meter>());
        let mut transmission = if rectangle.contains_point(&translation, &point_meter) {
            1.0
        } else {
            0.0
        };
        if matches!(self.aperture_type, ApertureType::Obstruction) {
            transmission = 1.0 - transmission;
        }
        transmission
    }
}
/// Configuration of a polygonal aperture defined by a given set of points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolygonConfig {
    points: Vec<Point2<f64>>,
    aperture_type: ApertureType,
}
impl PolygonConfig {
    /// Create a new polygonal aperture configuration by a set of given 2D points.
    ///
    /// The order of the points must follow the outline of the polygon. Otherwise intersections may occur.
    /// By default the aperture has the aperture type [`ApertureType::Hole`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the number of points is less than three, so that no polygon can be created.
    pub fn new(points: Vec<Point2<f64>>) -> OpmResult<Self> {
        if points.len() < 3 {
            return Err(OpossumError::Other("less than 3 points given".into()));
        }
        Ok(Self {
            points,
            aperture_type: ApertureType::default(),
        })
    }
}
impl Apodize for PolygonConfig {
    fn set_aperture_type(&mut self, aperture_type: ApertureType) {
        self.aperture_type = aperture_type;
    }
    fn apodize(&self, point: &Point2<Length>) -> f64 {
        let polygon = Polyline::new(self.points.clone(), None);
        let point_meter = Point2::<f64>::new(point.x.get::<meter>(), point.y.get::<meter>());
        let mut transmission = if polygon.contains_point(&Isometry2::identity(), &point_meter) {
            1.0
        } else {
            0.0
        };
        if matches!(self.aperture_type, ApertureType::Obstruction) {
            transmission = 1.0 - transmission;
        }
        transmission
    }
}
/// Configuration data for a Gaussian aperture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaussianConfig {
    sigma: (Length, Length),
    center: Point2<Length>,
    aperture_type: ApertureType,
}
impl GaussianConfig {
    /// Create a Gaussian aperture configurartion given by `(sigma_x, sigma_y)` as well as the center point.
    ///
    /// By default the aperture has the aperture type [`ApertureType::Hole`].
    /// # Errors
    ///
    /// This function will return an error if the given waists are negative and / or the center point is indefinite.
    pub fn new(sigma: (Length, Length), center: Point2<Length>) -> OpmResult<Self> {
        if sigma.0.is_normal()
            && sigma.0.is_sign_positive()
            && sigma.1.is_normal()
            && sigma.1.is_sign_positive()
            && center.coords[0].is_finite()
            && center.coords[1].is_finite()
        {
            Ok(Self {
                sigma,
                center,
                aperture_type: ApertureType::default(),
            })
        } else {
            Err(OpossumError::Other("parameters out of range".into()))
        }
    }
}
impl Apodize for GaussianConfig {
    fn set_aperture_type(&mut self, aperture_type: ApertureType) {
        self.aperture_type = aperture_type;
    }
    fn apodize(&self, point: &Point2<Length>) -> f64 {
        let x_c = self.center.coords[0];
        let y_c = self.center.coords[1];
        let x = point.coords[0];
        let y = point.coords[1];
        let mut transmission = (-0.5
            * (((x - x_c) / self.sigma.0).get::<ratio>().powi(2)
                + ((y - y_c) / self.sigma.1).get::<ratio>().powi(2)))
        .exp();
        if matches!(self.aperture_type, ApertureType::Obstruction) {
            transmission = 1.0 - transmission;
        }
        transmission
    }
}
/// Configuration of an aperture stack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackConfig {
    apertures: Vec<Aperture>,
    aperture_type: ApertureType,
}
impl StackConfig {
    /// Creates a new [`StackConfig`] by a given set of apertures.
    ///
    /// All aperture transmissions are multiplied, thus realizing a "subtrative" aperture. After that the transmission can be "inverted"
    /// (`transmission = 1.0 - transmission`) by setting the aperture type to [`ApertureType::Obstruction`].
    #[must_use]
    pub fn new(apertures: Vec<Aperture>) -> Self {
        Self {
            apertures,
            aperture_type: ApertureType::default(),
        }
    }
}
impl Apodize for StackConfig {
    fn set_aperture_type(&mut self, aperture_type: ApertureType) {
        self.aperture_type = aperture_type;
    }
    fn apodize(&self, point: &Point2<Length>) -> f64 {
        let mut transmission = 1.0;
        for a in &self.apertures {
            transmission *= a.apodization_factor(point);
        }
        if matches!(self.aperture_type, ApertureType::Obstruction) {
            transmission = 1.0 - transmission;
        }
        transmission
    }
}

#[cfg(test)]
mod test {
    use approx::assert_relative_eq;

    use super::*;
    #[test]
    fn macro_meter(){
        let meter1 = Length::new::<meter>(2.);
        let meter2 = meter!(2.);

    }
    #[test]
    fn ratio_test(){
        let x = Length::new::<meter>(1.);
        let xs = Length::new::<meter>(2.);

        assert_relative_eq!(x.get::<meter>()/xs.get::<meter>(), (x/xs).get::<ratio>());
    }
    #[test]
    fn default() {
        assert!(matches!(Aperture::default(), Aperture::None));
    }
    // #[test]
    // fn circle_config() {
    //     let center = Point2::new(0.0, 0.0);
    //     assert!(CircleConfig::new(1.0, center).is_ok());
    //     assert!(CircleConfig::new(0.0, center).is_err());
    //     assert!(CircleConfig::new(-1.0, center).is_err());
    //     assert!(CircleConfig::new(f64::NAN, center).is_err());
    //     assert!(CircleConfig::new(f64::INFINITY, center).is_err());
    // }
    // #[test]
    // fn rectangle_config() {
    //     let p = Point2::new(0.0, 0.0);
    //     assert!(RectangleConfig::new(2.0, 1.0, p).is_ok());
    //     assert!(RectangleConfig::new(0.0, 1.0, p).is_err());
    //     assert!(RectangleConfig::new(-1.0, 1.0, p).is_err());
    //     assert!(RectangleConfig::new(f64::NAN, 1.0, p).is_err());
    //     assert!(RectangleConfig::new(f64::INFINITY, 1.0, p).is_err());
    //     assert!(RectangleConfig::new(1.0, 0.0, p).is_err());
    //     assert!(RectangleConfig::new(1.0, -1.0, p).is_err());
    //     assert!(RectangleConfig::new(1.0, f64::NAN, p).is_err());
    //     assert!(RectangleConfig::new(1.0, f64::INFINITY, p).is_err());
    //     let p = Point2::new(f64::NAN, 0.0);
    //     assert!(RectangleConfig::new(2.0, 1.0, p).is_err());
    //     let p = Point2::new(f64::INFINITY, 0.0);
    //     assert!(RectangleConfig::new(2.0, 1.0, p).is_err());
    // }
    // #[test]
    // fn polygon_config() {
    //     let ok_points = vec![
    //         Point2::new(0.0, 0.0),
    //         Point2::new(2.0, 0.0),
    //         Point2::new(1.0, 1.0),
    //     ];
    //     assert!(PolygonConfig::new(ok_points).is_ok());
    //     let too_little_points = vec![Point2::new(0.0, 0.0), Point2::new(2.0, 0.0)];
    //     assert!(PolygonConfig::new(too_little_points).is_err());
    // }
    // #[test]
    // fn gaussian_config() {
    //     let p = Point2::new(0.0, 0.0);
    //     assert!(RectangleConfig::new(2.0, 1.0, p).is_ok());
    //     assert!(GaussianConfig::new((1.0, 1.0), p).is_ok());
    //     assert!(GaussianConfig::new((0.0, 1.0), p).is_err());
    //     assert!(GaussianConfig::new((-1.0, 1.0), p).is_err());
    //     assert!(GaussianConfig::new((1.0, 0.0), p).is_err());
    //     assert!(GaussianConfig::new((1.0, -1.0), p).is_err());
    //     assert!(GaussianConfig::new((f64::NAN, 1.0), p).is_err());
    //     assert!(GaussianConfig::new((f64::INFINITY, 1.0), p).is_err());
    //     assert!(GaussianConfig::new((1.0, f64::NAN), p).is_err());
    //     assert!(GaussianConfig::new((1.0, f64::INFINITY), p).is_err());
    //     let p = Point2::new(f64::NAN, 0.0);
    //     assert!(GaussianConfig::new((1.0, 1.0), p).is_err());
    //     let p = Point2::new(f64::INFINITY, 0.0);
    //     assert!(GaussianConfig::new((1.0, 1.0), p).is_err());
    // }
    // #[test]
    // fn binary_circle() {
    //     let c = CircleConfig::new(1.0, Point2::new(1.0, 1.0)).unwrap();
    //     let ap = Aperture::BinaryCircle(c);
    //     assert_eq!(ap.apodization_factor(&Point2::new(1.0, 1.0)), 1.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(1.0, 0.0)), 1.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(1.0, 2.0)), 1.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(2.0, 1.0)), 1.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(0.0, 1.0)), 1.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(0.0, 0.0)), 0.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(2.0, 2.0)), 0.0);
    //     let mut c = CircleConfig::new(1.0, Point2::new(1.0, 1.0)).unwrap();
    //     c.set_aperture_type(ApertureType::Obstruction);
    //     let ap = Aperture::BinaryCircle(c);
    //     assert_eq!(ap.apodization_factor(&Point2::new(1.0, 1.0)), 0.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(0.0, 0.0)), 1.0);
    // }
    // #[test]
    // fn binary_rectangle() {
    //     let r = RectangleConfig::new(1.0, 2.0, Point2::new(1.0, 1.0)).unwrap();
    //     let ap = Aperture::BinaryRectangle(r);
    //     assert_eq!(ap.apodization_factor(&Point2::new(1.0, 1.0)), 1.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(1.5, 1.0)), 1.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(1.5, 2.0)), 1.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(0.5, 2.0)), 1.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(0.5, 0.0)), 1.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(0.0, 0.0)), 0.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(1.0, 2.1)), 0.0);
    //     let mut r = RectangleConfig::new(1.0, 2.0, Point2::new(1.0, 1.0)).unwrap();
    //     r.set_aperture_type(ApertureType::Obstruction);
    //     let ap = Aperture::BinaryRectangle(r);
    //     assert_eq!(ap.apodization_factor(&Point2::new(1.0, 1.0)), 0.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(0.0, 0.0)), 1.0);
    // }
    // #[test]
    // fn binary_polygon() {
    //     let poly = PolygonConfig::new(vec![
    //         Point2::new(0.0, 0.0),
    //         Point2::new(1.0, 0.5),
    //         Point2::new(2.0, 0.0),
    //         Point2::new(1.0, 1.0),
    //     ])
    //     .unwrap();
    //     let ap = Aperture::BinaryPolygon(poly);
    //     assert_eq!(ap.apodization_factor(&Point2::new(0.0, 0.0)), 1.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(2.0, 0.0)), 1.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(1.0, 1.0)), 1.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(1.0, 0.0)), 0.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(2.0, 1.0)), 0.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(0.0, 1.0)), 0.0);
    //     let mut poly = PolygonConfig::new(vec![
    //         Point2::new(0.0, 0.0),
    //         Point2::new(2.0, 0.0),
    //         Point2::new(1.0, 1.0),
    //     ])
    //     .unwrap();
    //     poly.set_aperture_type(ApertureType::Obstruction);
    //     let ap = Aperture::BinaryPolygon(poly);
    //     assert_eq!(ap.apodization_factor(&Point2::new(0.0, 0.0)), 0.0);
    //     assert_eq!(ap.apodization_factor(&Point2::new(2.0, 1.0)), 1.0);
    // }
    // #[test]
    // fn gaussian() {
    //     let g = GaussianConfig::new((1.0, 1.0), Point2::new(1.0, 1.0)).unwrap();
    //     let ap = Aperture::Gaussian(g);
    //     assert_eq!(ap.apodization_factor(&Point2::new(1.0, 1.0)), 1.0);
    //     assert_eq!(
    //         ap.apodization_factor(&Point2::new(0.0, 0.0)),
    //         1.0 / 1.0_f64.exp()
    //     );
    //     let mut g = GaussianConfig::new((1.0, 1.0), Point2::new(1.0, 1.0)).unwrap();
    //     g.set_aperture_type(ApertureType::Obstruction);
    //     let ap = Aperture::Gaussian(g);
    //     assert_eq!(ap.apodization_factor(&Point2::new(1.0, 1.0)), 0.0);
    //     assert_eq!(
    //         ap.apodization_factor(&Point2::new(0.0, 0.0)),
    //         1.0 - 1.0 / 1.0_f64.exp()
    //     );
    // }
    // #[test]
    // fn stack() {
    //     let r = RectangleConfig::new(1.0, 1.0, Point2::new(0.5, 0.5)).unwrap();
    //     let r_ap = Aperture::BinaryRectangle(r);
    //     let c = CircleConfig::new(1.0, Point2::new(0.0, 0.0)).unwrap();
    //     let c_ap = Aperture::BinaryCircle(c);
    //     let s = StackConfig::new(vec![r_ap, c_ap]);
    //     let s_ap = Aperture::Stack(s);
    //     assert_eq!(s_ap.apodization_factor(&Point2::new(0.0, 0.0)), 1.0);
    //     assert_eq!(s_ap.apodization_factor(&Point2::new(1.0, 0.0)), 1.0);
    //     assert_eq!(s_ap.apodization_factor(&Point2::new(0.0, 1.0)), 1.0);
    //     assert_eq!(s_ap.apodization_factor(&Point2::new(1.0, 1.0)), 0.0);
    //     assert_eq!(s_ap.apodization_factor(&Point2::new(-1.0, 0.0)), 0.0);
    //     assert_eq!(s_ap.apodization_factor(&Point2::new(0.0, -1.0)), 0.0);
    //     let r = RectangleConfig::new(1.0, 1.0, Point2::new(0.5, 0.5)).unwrap();
    //     let r_ap = Aperture::BinaryRectangle(r);
    //     let c = CircleConfig::new(1.0, Point2::new(0.0, 0.0)).unwrap();
    //     let c_ap = Aperture::BinaryCircle(c);
    //     let mut s = StackConfig::new(vec![r_ap, c_ap]);
    //     s.set_aperture_type(ApertureType::Obstruction);
    //     let s_ap = Aperture::Stack(s);
    //     assert_eq!(s_ap.apodization_factor(&Point2::new(0.0, 0.0)), 0.0);
    //     assert_eq!(s_ap.apodization_factor(&Point2::new(1.0, 1.0)), 1.0);
    // }
}
