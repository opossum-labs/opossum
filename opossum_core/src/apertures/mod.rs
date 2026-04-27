#![warn(missing_docs)]
//! Module for handling optical (2D) apertures
//!
//! An [`Aperture`] commonly defines the shape of an optical element which transmits or obstructs an incoming optical ray.
//! Currently there are "binary" shapes which either fully transmits or fully blocks a ray at a given point. Furthermore, an variable
//! transmission Gaussian aperture exists. Finally a set of apertures can be stacked on top of each other in order form aperture shapes
//! of higher complexity.
//!
//! Apertures a defined by their respective configuration struct. For the calculation the function
//! [`apodize`](Aperture::apodize()) is used.
//! ```rust
//! use opossum_core::prelude::*;
//!
//! let ap = Aperture::new_circle(millimeter!(1.0), millimeter!(1.0, 1.0), ApertureType::Hole).unwrap();
//! assert_eq!(ap.apodize(&millimeter!(1.0,1.0)), 1.0);
//! assert_eq!(ap.apodize(&millimeter!(0.0,0.0)), 0.0);
//! ```
//! Furthermore, each aperture can act as a "hole" or as an "obstruction".
//! ```rust
//! use opossum_core::prelude::*;
//!
//! let ap = Aperture::new_circle(millimeter!(1.0), millimeter!(1.0, 1.0), ApertureType::Obstruction).unwrap();
//! assert_eq!(ap.apodize(&millimeter!(1.0, 1.0)), 0.0);
//! assert_eq!(ap.apodize(&millimeter!(0.0, 0.0)), 1.0);
//! ```
mod circle;
mod gaussian;
mod polygon;
mod rectangle;
mod stack;

use crate::{
    error::OpmResult,
    properties::Proptype,
    reporting::plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    utils::{default_from_name::DefaultFromName, math_distribution_functions::ellipse},
    generic_validators::ValidateTrait
};
use core::f64;
use std::fmt::Display;
use nalgebra::{Matrix2xX, MatrixXx2, Point2};
use opm_macros_lib::EnsureValidated;
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use strum::EnumIter;
use uom::si::{f64::Length, length::millimeter};

pub use circle::CircleShape;
pub use gaussian::GaussianShape;
pub use polygon::PolygonConfig;
pub use rectangle::RectangleShape;
pub use stack::StackShape;
use utoipa::ToSchema;

/// The apodization type of an [`Aperture`].
///
/// Each aperture can act as a "hole" or "obstruction"
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema, EnumIter, EnsureValidated)]
pub enum ApertureType {
    /// the [`Aperture`] shape acts as a hole. The inner part of the shape is transparent.
    #[default]
    Hole,
    /// the [`Aperture`] shape represents an obstruction. The inner part of the shape is opaque.
    Obstruction,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema, EnsureValidated)]
pub struct Aperture{
    shape: ApertureShape,
    aperture_type: ApertureType
    // isometry: Isometry
}

impl Aperture {
    pub fn new(shape: ApertureShape, aperture_type: ApertureType) -> Self {
        Self { shape, aperture_type }
    }
    pub fn apodize(&self, point: &Point2<Length>) -> f64 {
        let base_transmission = self.shape.apodize(point);
        if matches!(self.aperture_type, ApertureType::Obstruction) {
            1.0 - base_transmission
        } else {
            base_transmission
        }
    }
    pub fn shape(&self) -> &ApertureShape {
        &self.shape
    }
    pub fn aperture_type(&self) -> &ApertureType {
        &self.aperture_type
    }
    /// Create a new circular aperture.
    ///
    /// # Errors
    ///
    /// This function will return an error if the parameters of the aperture creation are invalid.
    pub fn new_circle(
        radius: Length,
        center: Point2<Length>,
        aperture_type: ApertureType,
    ) -> OpmResult<Self> {
        let config = CircleShape::new(radius, center)?;
        Ok(Self::new(ApertureShape::BinaryCircle(config), aperture_type))
    }
    /// Create a new retangular aperture.
    ///
    /// # Errors
    ///
    /// This function will return an error if the parameters of the aperture creation are invalid.
    pub fn new_rectangle(
        width: Length,
        height: Length,
        center: Point2<Length>,
        aperture_type: ApertureType,
    ) -> OpmResult<Self> {
        let config = RectangleShape::new(width, height, center)?;
        Ok(Self::new(ApertureShape::BinaryRectangle(config), aperture_type))
    }
    /// Create a new Gaussian aperture.
    ///
    /// # Errors
    ///
    /// This function will return an error if the parameters of the aperture creation are invalid.
    pub fn new_gaussian(
        sigma: (Length, Length),
        center: Point2<Length>,
        aperture_type: ApertureType,
    ) -> OpmResult<Self> {
        let config = GaussianShape::new(sigma, center)?;
        Ok(Self::new(ApertureShape::Gaussian(config), aperture_type))
    }
    /// Create a new polygon aperture.
    ///
    /// # Errors
    ///
    /// This function will return an error if the parameters of the aperture creation are invalid.
    pub fn new_polygon(
        points: Vec<Point2<Length>>,
        aperture_type: ApertureType,
    ) -> OpmResult<Self> {
        let config = PolygonConfig::new(points)?;
        Ok(Self::new(ApertureShape::BinaryPolygon(config), aperture_type))
    }
    /// Create a new stack aperture.
    #[must_use]
    pub fn new_stack(apertures: Vec<Self>, aperture_type: ApertureType) -> OpmResult<Self> {
        let config = StackShape::new(apertures)?;
        Ok(Self::new(ApertureShape::Stack(config), aperture_type))
    }

    /// Check if the aperture-shape is [`ApertureShape::Open`]
    pub fn is_none(&self) -> bool {
        self.shape.is_none()
    }
}

/// Different aperture types
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema, EnumIter, EnsureValidated)]
pub enum ApertureShape {
    /// completely transparent aperture. This is the default.
    #[default]
    Open,
    /// binary (either transparent or opaque) circular aperture defined by a radius and center point
    BinaryCircle(CircleShape),
    /// binary (either transparent or opaque) rectangular aperture defined by width and height as well as its center point
    BinaryRectangle(RectangleShape),
    /// binary (either transparent or opaque) polygonial aperture defined by a set of 2D points. This polygon can also be
    /// non-convex but should not intersect.
    BinaryPolygon(PolygonConfig),
    /// variable transmission aperture using a 2D Gaussian function.
    Gaussian(GaussianShape),
    /// a stack of an arbitrary number of the above apertures. The transmission factor at a given point is the
    /// product of all indiviual aperture on the stack (subtractive apodization).
    Stack(StackShape),
}
impl DefaultFromName for ApertureShape {}

impl Display for ApertureShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApertureShape::Open => write!(f, "Open"),
            ApertureShape::BinaryCircle(_) => write!(f, "Circle"),
            ApertureShape::BinaryRectangle(_) => write!(f, "Rectangle"),
            ApertureShape::BinaryPolygon(_) => write!(f, "Polygon"),
            ApertureShape::Gaussian(_) => write!(f, "Gaussian"),
            ApertureShape::Stack(_) => write!(f, "Stacked apertures"),
        }
    }
}

impl ApertureShape {

    #[must_use]
    /// Check if the aperture is [`ApertureShape::Open`]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::Open)
    }
    /// Calculate the transmission factor of a given point on the [`Aperture`]. The value is in the range (0.0..=1.0)
    /// 0.0 is fully opaque, 1.0 fully transparent.
    #[must_use]
    fn apodize(&self, point: &Point2<Length>) -> f64 {
        // Resolve both transmission and type in a single match for clarity and correctness
        match self {
            Self::Open => (1.0),
            Self::BinaryCircle(shape) => shape.transmission_factor(point),
            Self::BinaryRectangle(shape) => shape.transmission_factor(point),
            Self::BinaryPolygon(shape) => shape.transmission_factor(point),
            Self::Gaussian(shape) => shape.transmission_factor(point),
            Self::Stack(apertures) => 
                apertures
                    .apertures()
                    .iter()
                    .fold(1.0, |acc, ap| acc * ap.apodize(point)),
        }
    }
}
impl From<ApertureShape> for Proptype {
    fn from(value: ApertureShape) -> Self {
        Self::Aperture(value)
    }
}

impl From<RectangleShape> for ApertureShape {
    fn from(rect: RectangleShape) -> Self {
        Self::BinaryRectangle(rect)
    }
}

/// Trait for the calculation of the transmission factor for each shape.
pub trait Shape {
    /// Calculate the transmission factor (always treated as aperture type `hole`).
    fn transmission_factor(&self, point: &Point2<Length>) -> f64;
}

impl Plottable for Aperture {
    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let plt_series_opt = match plt_type {
            PlotType::Line2D(_) | PlotType::Scatter2D(_) => match &self.shape {
                ApertureShape::Open => None,
                ApertureShape::BinaryCircle(conf) => Some(stack::plot_circle(conf)),
                ApertureShape::BinaryRectangle(conf) => {
                    let center_x = conf.center().x.get::<millimeter>();
                    let center_y = conf.center().y.get::<millimeter>();
                    let half_width = conf.width().get::<millimeter>() / 2.;
                    let half_height = conf.height().get::<millimeter>() / 2.;
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
                ApertureShape::BinaryPolygon(conf) => {
                    let mut xy_data = MatrixXx2::from_element(conf.points().len(), 0.);
                    for (row, p) in conf.points().iter().enumerate() {
                        xy_data[(row, 0)] = p.x.get::<millimeter>();
                        xy_data[(row, 1)] = p.y.get::<millimeter>();
                    }
                    Some(vec![PlotSeries::new(
                        &PlotData::Dim2 { xy_data },
                        RGBAColor(0, 0, 0, 1.),
                        Some("Aperture".to_owned()),
                    )])
                }
                ApertureShape::Gaussian(conf) => {
                    let circle_points = ellipse(
                        (
                            conf.center().x.get::<millimeter>(),
                            conf.center().y.get::<millimeter>(),
                        ),
                        (
                            conf.sigma().0.get::<millimeter>() * 2.,
                            conf.sigma().1.get::<millimeter>() * 2.,
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
                ApertureShape::Stack(conf) => {
                    let mut aperture_series_vec =
                        Vec::<PlotSeries>::with_capacity(conf.apertures().len());
                    for aperture in conf.apertures() {
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
    #[test]
    fn default() {
        assert!(matches!(ApertureShape::default(), ApertureShape::Open));
    }
    #[test]
    fn test_new_circle() {
        let center = meter!(0.0, 0.0);
        // Valid radius
        assert!(Aperture::new_circle(meter!(1.0), center, ApertureType::Hole).is_ok());
        // Invalid radius (negative)
        assert!(Aperture::new_circle(meter!(-1.0), center, ApertureType::Hole).is_err());
    }

    #[test]
    fn test_new_rectangle() {
        let center = meter!(0.0, 0.0);
        // Valid dimensions
        assert!(
            Aperture::new_rectangle(meter!(1.0), meter!(1.0), center, ApertureType::Hole).is_ok()
        );
        // Invalid height
        assert!(
            Aperture::new_rectangle(meter!(1.0), meter!(0.0), center, ApertureType::Hole).is_err()
        );
    }

    #[test]
    fn test_new_gaussian() {
        let center = meter!(0.0, 0.0);
        // Valid sigma
        assert!(
            Aperture::new_gaussian((meter!(1.0), meter!(1.0)), center, ApertureType::Hole).is_ok()
        );
        // Invalid sigma (zero)
        assert!(
            Aperture::new_gaussian((meter!(0.0), meter!(1.0)), center, ApertureType::Hole).is_err()
        );
    }

    #[test]
    fn test_new_polygon() {
        // Valid triangle
        let points = vec![meter!(0.0, 0.0), meter!(1.0, 0.0), meter!(0.0, 1.0)];
        assert!(Aperture::new_polygon(points, ApertureType::Hole).is_ok());
        // Invalid polygon (too few points)
        let points_too_few = vec![meter!(0.0, 0.0), meter!(1.0, 0.0)];
        assert!(Aperture::new_polygon(points_too_few, ApertureType::Hole).is_err());
    }

    #[test]
    fn test_new_stack() {
        let circle =
            Aperture::new_circle(meter!(1.0), meter!(0.0, 0.0), ApertureType::Hole).unwrap();
        let rect = Aperture::new_rectangle(
            meter!(1.0),
            meter!(1.0),
            meter!(0.0, 0.0),
            ApertureType::Hole,
        )
        .unwrap();

        // Stack returns Self directly, not OpmResult
        let stack = Aperture::new_stack(vec![circle, rect], ApertureType::Obstruction).unwrap();

        if let ApertureShape::Stack(config) = stack.shape {
            assert_eq!(config.apertures().len(), 2);
            assert_eq!(stack.aperture_type, ApertureType::Obstruction);
        } else {
            panic!("Expected Aperture::Stack variant");
        }
    }

    #[test]
    fn test_is_none() {
        assert!(ApertureShape::Open.is_none());
        let circle =
            Aperture::new_circle(meter!(1.0), meter!(0.0, 0.0), ApertureType::Hole).unwrap();
        assert!(!circle.is_none());
    }
    #[test]
    fn test_obstruction_logic() {
        // A circle as a hole (default)
        let hole = Aperture::new_circle(meter!(1.0), meter!(0.0, 0.0), ApertureType::Hole).unwrap();
        // A circle as an obstruction
        let block =
            Aperture::new_circle(meter!(1.0), meter!(0.0, 0.0), ApertureType::Obstruction).unwrap();

        let p_inside = meter!(0.5, 0.0);
        let p_outside = meter!(2.0, 0.0);

        assert_eq!(hole.apodize(&p_inside), 1.0);
        assert_eq!(hole.apodize(&p_outside), 0.0);

        assert_eq!(block.apodize(&p_inside), 0.0);
        assert_eq!(block.apodize(&p_outside), 1.0);
    }
}
