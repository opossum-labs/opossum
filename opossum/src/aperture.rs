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
//! use opossum::{millimeter, aperture::{Aperture, ApertureType, CircleConfig}};
//! use uom::si::{f64::Length, length::millimeter};
//!
//! let c = CircleConfig::new(millimeter!(1.0), millimeter!(1.0, 1.0)).unwrap();
//! let ap = Aperture::BinaryCircle(c);
//! assert_eq!(ap.apodization_factor(&millimeter!(1.0,1.0)), 1.0);
//! assert_eq!(ap.apodization_factor(&millimeter!(0.0,0.0)), 0.0);
//! ```
//! Furthermore, each aperture can act as a "hole" or as an "obstruction". By default,
//! all configurations are created as "holes".
//! ```rust
//! use nalgebra::Point2;
//! use opossum::{millimeter, aperture::{Aperture, ApertureType, CircleConfig, Apodize}};
//! use uom::si::{f64::Length, length::millimeter};
//!
//! let mut c = CircleConfig::new(millimeter!(1.0), millimeter!(1.0, 1.0)).unwrap();
//! c.set_aperture_type(ApertureType::Obstruction);
//! let ap = Aperture::BinaryCircle(c);
//! assert_eq!(ap.apodization_factor(&millimeter!(1.0, 1.0)), 0.0);
//! assert_eq!(ap.apodization_factor(&millimeter!(0.0, 0.0)), 1.0);
//! ```

use crate::{
    error::{OpmResult, OpossumError},
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::Proptype,
    utils::math_distribution_functions::ellipse,
};
use core::f64;
use earcutr::earcut;
use nalgebra::{Isometry2, Matrix2xX, MatrixXx2, Point2, Vector2};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::Length,
    length::{meter, millimeter},
    ratio::ratio,
};

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
    #[must_use]
    /// Check if the aperture is [`Aperture::None`]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
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

    ///return the radius of this [`CircleConfig`]
    #[must_use]
    pub fn radius(&self) -> &Length {
        &self.radius
    }
}
impl Apodize for CircleConfig {
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
        let mut transmission = if point_transformed
            .y
            .mul_add(point_transformed.y, point_transformed.x.powi(2))
            <= self.radius.get::<meter>().powi(2)
        {
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
/// Configuration of a polygonal aperture defined by a given set of points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolygonConfig {
    points: Vec<Point2<Length>>,
    aperture_type: ApertureType,
    triangle_indices: Vec<Vec<usize>>,
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
    pub fn new(points: Vec<Point2<Length>>) -> OpmResult<Self> {
        if points.len() < 3 {
            return Err(OpossumError::Other("less than 3 points given".into()));
        }
        Ok(Self {
            triangle_indices: Self::triangulate(&points)?,
            points,
            aperture_type: ApertureType::default(),
        })
    }

    fn triangulate(points: &[Point2<Length>]) -> OpmResult<Vec<Vec<usize>>> {
        let polygon_vertices_flat = points
            .iter()
            .flat_map(|p| vec![p.x.get::<meter>(), p.y.get::<meter>()])
            .collect::<Vec<f64>>();

        let triangulated_indices = earcut(polygon_vertices_flat.as_slice(), &[], 2)
            .map_err(|e| OpossumError::Other(format!("Triangulation of polygon failed:{e}")))?;
        let mut chunked_indices = Vec::<Vec<usize>>::with_capacity(triangulated_indices.len() / 3);
        for chunk in triangulated_indices.chunks(3) {
            chunked_indices.push(Vec::<usize>::from(chunk));
        }
        Ok(chunked_indices)
    }

    /// checks, if a point lies within this [`PolygonConfig`]
    /// # Panics
    /// This function panics if the triangulation fails
    #[must_use]
    pub fn in_polygon(&self, point: &Point2<Length>) -> bool {
        let mut in_polygon = false;
        for tri in &self.triangle_indices {
            let p1 = self.points[tri[0]];
            let p2 = self.points[tri[1]];
            let p3 = self.points[tri[2]];

            let denominator =
                (p2[1] - p3[1]).mul_add(p1[0] - p3[0], (p3[0] - p2[0]) * (p1[1] - p3[1]));
            let a = (((p2[1] - p3[1])
                .mul_add(point.x - p3[0], (p3[0] - p2[0]) * (point.y - p3[1])))
                / denominator)
                .value;
            let b = (((p3[1] - p1[1])
                .mul_add(point.x - p3[0], (p1[0] - p3[0]) * (point.y - p3[1])))
                / denominator)
                .value;
            let c = 1. - a - b;

            if (0. ..=1.).contains(&a) && (0. ..=1.).contains(&b) && (0. ..=1.).contains(&c) {
                in_polygon = true;
                break;
            }
        }
        in_polygon
    }
}
impl Apodize for PolygonConfig {
    fn set_aperture_type(&mut self, aperture_type: ApertureType) {
        self.aperture_type = aperture_type;
    }
    fn apodize(&self, point: &Point2<Length>) -> f64 {
        let mut transmission = if self.in_polygon(point) { 1.0 } else { 0.0 };
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
            * (((x - x_c) / self.sigma.0).get::<ratio>().mul_add(
                ((x - x_c) / self.sigma.0).get::<ratio>(),
                ((y - y_c) / self.sigma.1).get::<ratio>().powi(2),
            )))
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
fn plot_circle(conf: &CircleConfig) -> Vec<PlotSeries> {
    let circle_points = ellipse(
        (
            conf.center.x.get::<millimeter>(),
            conf.center.y.get::<millimeter>(),
        ),
        (
            conf.radius.get::<millimeter>(),
            conf.radius.get::<millimeter>(),
        ),
        100,
    )
    .unwrap();
    let plt_dat = PlotData::Dim2 {
        xy_data: Matrix2xX::from_vec(
            circle_points
                .iter()
                .flat_map(|p| vec![p.x, p.y])
                .collect::<Vec<f64>>(),
        )
        .transpose(),
    };
    vec![PlotSeries::new(
        &plt_dat,
        RGBAColor(0, 0, 0, 1.),
        Some("Aperture".to_owned()),
    )]
}
impl Plottable for Aperture {
    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let plt_series_opt = match plt_type {
            PlotType::Line2D(_) | PlotType::Scatter2D(_) => match self {
                Self::None => None,
                Self::BinaryCircle(conf) => Some(plot_circle(conf)),
                Self::BinaryRectangle(conf) => {
                    let center_x = conf.center.x.get::<millimeter>();
                    let center_y = conf.center.y.get::<millimeter>();
                    let half_width = conf.width.get::<millimeter>() / 2.;
                    let half_height = conf.height.get::<millimeter>() / 2.;
                    let plt_dat = PlotData::Dim2 {
                        xy_data: Matrix2xX::<f64>::from_vec(vec![
                            center_x - half_width,
                            center_y + half_height,
                            center_x - half_width,
                            center_y - half_height,
                            center_x + half_width,
                            center_y - half_height,
                            center_x + half_width,
                            center_y + half_height,
                        ])
                        .transpose(),
                    };

                    let series_label = if legend {
                        Some("Aperture".to_owned())
                    } else {
                        None
                    };
                    Some(vec![PlotSeries::new(
                        &plt_dat,
                        RGBAColor(0, 0, 0, 1.),
                        series_label,
                    )])
                }

                Self::BinaryPolygon(conf) => {
                    let mut xy_data = MatrixXx2::from_element(conf.points.len(), 0.);
                    for (row, p) in conf.points.iter().enumerate() {
                        xy_data[(row, 0)] = p.x.get::<millimeter>();
                        xy_data[(row, 1)] = p.y.get::<millimeter>();
                    }
                    Some(vec![PlotSeries::new(
                        &PlotData::Dim2 { xy_data },
                        RGBAColor(0, 0, 0, 1.),
                        Some("Aperture".to_owned()),
                    )])
                }
                Self::Gaussian(conf) => {
                    let circle_points = ellipse(
                        (
                            conf.center.x.get::<millimeter>(),
                            conf.center.y.get::<millimeter>(),
                        ),
                        (
                            conf.sigma.0.get::<millimeter>() * 2.,
                            conf.sigma.1.get::<millimeter>() * 2.,
                        ),
                        100,
                    )?;
                    let xy_data = Matrix2xX::from_vec(
                        circle_points
                            .iter()
                            .flat_map(|p| vec![p.x, p.y])
                            .collect::<Vec<f64>>(),
                    )
                    .transpose();
                    Some(vec![PlotSeries::new(
                        &PlotData::Dim2 { xy_data },
                        RGBAColor(0, 0, 0, 1.),
                        Some("Gaussian Aperture 2-sigma".to_owned()),
                    )])
                }
                Self::Stack(conf) => {
                    let mut aperture_series_vec =
                        Vec::<PlotSeries>::with_capacity(conf.apertures.len());
                    for aperture in &conf.apertures {
                        if let Some(plt_series_vec) = aperture.get_plot_series(plt_type, legend)? {
                            aperture_series_vec.extend(plt_series_vec);
                        }
                    }
                    Some(aperture_series_vec)
                }
            },
            _ => None,
        };

        Ok(plt_series_opt)
    }

    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("position in mm".into()))?
            .set(&PlotArgs::YLabel("position in mm".into()))?
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotSize((800, 800)))?;
        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::Line2D(plt_params.clone())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::meter;
    use approx::assert_relative_eq;

    #[test]
    fn ratio_test() {
        let x = meter!(1.);
        let xs = meter!(2.);

        assert_relative_eq!(
            x.get::<meter>() / xs.get::<meter>(),
            (x / xs).get::<ratio>()
        );
    }
    #[test]
    fn default() {
        assert!(matches!(Aperture::default(), Aperture::None));
    }
    #[test]
    fn circle_config() {
        let center = meter!(0.0, 0.0);
        assert!(CircleConfig::new(meter!(1.0), center).is_ok());
        assert!(CircleConfig::new(meter!(0.0), center).is_err());
        assert!(CircleConfig::new(meter!(-1.0), center).is_err());
        assert!(CircleConfig::new(meter!(f64::NAN), center).is_err());
        assert!(CircleConfig::new(meter!(f64::INFINITY), center).is_err());
    }
    #[test]
    fn rectangle_config() {
        let p = meter!(0.0, 0.0);
        assert!(RectangleConfig::new(meter!(2.0), meter!(1.0), p).is_ok());
        assert!(RectangleConfig::new(meter!(0.0), meter!(1.0), p).is_err());
        assert!(RectangleConfig::new(meter!(-1.0), meter!(1.0), p).is_err());
        assert!(RectangleConfig::new(meter!(f64::NAN), meter!(1.0), p).is_err());
        assert!(RectangleConfig::new(meter!(f64::INFINITY), meter!(1.0), p).is_err());
        assert!(RectangleConfig::new(meter!(1.0), meter!(0.0), p).is_err());
        assert!(RectangleConfig::new(meter!(1.0), meter!(-1.0), p).is_err());
        assert!(RectangleConfig::new(meter!(1.0), meter!(f64::NAN), p).is_err());
        assert!(RectangleConfig::new(meter!(1.0), meter!(f64::INFINITY), p).is_err());
        let p = meter!(f64::NAN, 0.0);
        assert!(RectangleConfig::new(meter!(2.0), meter!(1.0), p).is_err());
        let p = meter!(f64::INFINITY, 0.0);
        assert!(RectangleConfig::new(meter!(2.0), meter!(1.0), p).is_err());
    }
    #[test]
    fn polygon_config() {
        let ok_points = vec![meter!(0.0, 0.0), meter!(2.0, 0.0), meter!(1.0, 1.0)];
        assert!(PolygonConfig::new(ok_points).is_ok());
        let too_little_points = vec![meter!(0.0, 0.0), meter!(2.0, 0.0)];
        assert!(PolygonConfig::new(too_little_points).is_err());
    }
    #[test]
    fn gaussian_config() {
        let p = meter!(0.0, 0.0);
        assert!(RectangleConfig::new(meter!(2.0), meter!(1.0), p).is_ok());
        assert!(GaussianConfig::new((meter!(1.0), meter!(1.0)), p).is_ok());
        assert!(GaussianConfig::new((meter!(0.0), meter!(1.0)), p).is_err());
        assert!(GaussianConfig::new((meter!(-1.0), meter!(1.0)), p).is_err());
        assert!(GaussianConfig::new((meter!(1.0), meter!(0.0)), p).is_err());
        assert!(GaussianConfig::new((meter!(1.0), meter!(-1.0)), p).is_err());
        assert!(GaussianConfig::new((meter!(f64::NAN), meter!(1.0)), p).is_err());
        assert!(GaussianConfig::new((meter!(f64::INFINITY), meter!(1.0)), p).is_err());
        assert!(GaussianConfig::new((meter!(1.0), meter!(f64::NAN)), p).is_err());
        assert!(GaussianConfig::new((meter!(1.0), meter!(f64::INFINITY)), p).is_err());
        let p = meter!(f64::NAN, 0.0);
        assert!(GaussianConfig::new((meter!(1.0), meter!(1.0)), p).is_err());
        let p = meter!(f64::INFINITY, 0.0);
        assert!(GaussianConfig::new((meter!(1.0), meter!(1.0)), p).is_err());
    }
    #[test]
    fn binary_circle() {
        let c = CircleConfig::new(meter!(1.0), meter!(1.0, 1.0)).unwrap();
        let ap = Aperture::BinaryCircle(c);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 1.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 0.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 2.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(2.0, 1.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 1.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 0.0)), 0.0);
        assert_eq!(ap.apodization_factor(&meter!(2.0, 2.0)), 0.0);
        let mut c = CircleConfig::new(meter!(1.0), meter!(1.0, 1.0)).unwrap();
        c.set_aperture_type(ApertureType::Obstruction);
        let ap = Aperture::BinaryCircle(c);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 1.0)), 0.0);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 0.0)), 1.0);
    }
    #[test]
    fn binary_rectangle() {
        let r = RectangleConfig::new(meter!(1.0), meter!(2.0), meter!(1.0, 1.0)).unwrap();
        let ap = Aperture::BinaryRectangle(r);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 1.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(1.5, 1.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(1.5, 2.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(0.5, 2.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(0.5, 0.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 0.0)), 0.0);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 2.1)), 0.0);
        let mut r = RectangleConfig::new(meter!(1.0), meter!(2.0), meter!(1.0, 1.0)).unwrap();
        r.set_aperture_type(ApertureType::Obstruction);
        let ap = Aperture::BinaryRectangle(r);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 1.0)), 0.0);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 0.0)), 1.0);
    }
    #[test]
    fn binary_polygon() {
        let poly = PolygonConfig::new(vec![
            meter!(0.0, 0.0),
            meter!(1.0, 0.5),
            meter!(2.0, 0.0),
            meter!(1.0, 1.0),
        ])
        .unwrap();
        let ap = Aperture::BinaryPolygon(poly);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 0.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(2.0, 0.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 1.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 0.0)), 0.0);
        assert_eq!(ap.apodization_factor(&meter!(2.0, 1.0)), 0.0);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 1.0)), 0.0);
        let mut poly =
            PolygonConfig::new(vec![meter!(0.0, 0.0), meter!(2.0, 0.0), meter!(1.0, 1.0)]).unwrap();
        poly.set_aperture_type(ApertureType::Obstruction);
        let ap = Aperture::BinaryPolygon(poly);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 0.0)), 0.0);
        assert_eq!(ap.apodization_factor(&meter!(2.0, 1.0)), 1.0);
    }
    #[test]
    fn gaussian() {
        let g = GaussianConfig::new((meter!(1.0), meter!(1.0)), meter!(1.0, 1.0)).unwrap();
        let ap = Aperture::Gaussian(g);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 1.0)), 1.0);
        assert_eq!(
            ap.apodization_factor(&meter!(0.0, 0.0)),
            1.0 / 1.0_f64.exp()
        );
        let mut g = GaussianConfig::new((meter!(1.0), meter!(1.0)), meter!(1.0, 1.0)).unwrap();
        g.set_aperture_type(ApertureType::Obstruction);
        let ap = Aperture::Gaussian(g);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 1.0)), 0.0);
        assert_eq!(
            ap.apodization_factor(&meter!(0.0, 0.0)),
            1.0 - 1.0 / 1.0_f64.exp()
        );
    }
    #[test]
    fn stack() {
        let r = RectangleConfig::new(meter!(1.0), meter!(1.0), meter!(0.5, 0.5)).unwrap();
        let r_ap = Aperture::BinaryRectangle(r);
        let c = CircleConfig::new(meter!(1.0), meter!(0.0, 0.0)).unwrap();
        let c_ap = Aperture::BinaryCircle(c);
        let s = StackConfig::new(vec![r_ap, c_ap]);
        let s_ap = Aperture::Stack(s);
        assert_eq!(s_ap.apodization_factor(&meter!(0.0, 0.0)), 1.0);
        assert_eq!(s_ap.apodization_factor(&meter!(1.0, 0.0)), 1.0);
        assert_eq!(s_ap.apodization_factor(&meter!(0.0, 1.0)), 1.0);
        assert_eq!(s_ap.apodization_factor(&meter!(1.0, 1.0)), 0.0);
        assert_eq!(s_ap.apodization_factor(&meter!(-1.0, 0.0)), 0.0);
        assert_eq!(s_ap.apodization_factor(&meter!(0.0, -1.0)), 0.0);
        let r = RectangleConfig::new(meter!(1.0), meter!(1.0), meter!(0.5, 0.5)).unwrap();
        let r_ap = Aperture::BinaryRectangle(r);
        let c = CircleConfig::new(meter!(1.0), meter!(0.0, 0.0)).unwrap();
        let c_ap = Aperture::BinaryCircle(c);
        let mut s = StackConfig::new(vec![r_ap, c_ap]);
        s.set_aperture_type(ApertureType::Obstruction);
        let s_ap = Aperture::Stack(s);
        assert_eq!(s_ap.apodization_factor(&meter!(0.0, 0.0)), 0.0);
        assert_eq!(s_ap.apodization_factor(&meter!(1.0, 1.0)), 1.0);
    }
}
