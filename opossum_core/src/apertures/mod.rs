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
//! use opossum_core::{millimeter, apertures::{Aperture, ApertureType, CircleConfig}};
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
//! use opossum_core::{millimeter, apertures::{Aperture, ApertureType, CircleConfig, Apodize}};
//! use uom::si::{f64::Length, length::millimeter};
//!
//! let mut c = CircleConfig::new(millimeter!(1.0), millimeter!(1.0, 1.0)).unwrap();
//! c.set_aperture_type(ApertureType::Obstruction);
//! let ap = Aperture::BinaryCircle(c);
//! assert_eq!(ap.apodization_factor(&millimeter!(1.0, 1.0)), 0.0);
//! assert_eq!(ap.apodization_factor(&millimeter!(0.0, 0.0)), 1.0);
//! ```
mod circle;
mod gaussian;
mod polygon;
mod rectangle;
mod stack;

use crate::{
    error::OpmResult,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::Proptype,
    utils::math_distribution_functions::ellipse,
};
use core::f64;
use nalgebra::{Matrix2xX, MatrixXx2, Point2};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::millimeter};

pub use circle::CircleShape;
pub use gaussian::GaussianShape;
pub use polygon::PolygonConfig;
pub use rectangle::RectangleShape;
pub use stack::StackConfig;

/// The apodization type of an [`Aperture`].
///
/// Each aperture can act as a "hole" or "obstruction"
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApertureType {
    /// the [`Aperture`] shape acts as a hole. The inner part of the shape is transparent.
    #[default]
    Hole,
    /// the [`Aperture`] shape represents an obstruction. The inner part of the shape is opaque.
    Obstruction,
}

/// Different aperture types
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Aperture {
    /// completely transparent aperture. This is the default.
    #[default]
    None,
    /// binary (either transparent or opaque) circular aperture defined by a radius and center point
    BinaryCircle(CircleShape, ApertureType),
    /// binary (either transparent or opaque) rectangular aperture defined by width and height as well as its center point
    BinaryRectangle(RectangleShape, ApertureType),
    /// binary (either transparent or opaque) polygonial aperture defined by a set of 2D points. This polygon can also be
    /// non-convex but should not intersect.
    BinaryPolygon(PolygonConfig, ApertureType),
    /// variable transmission aperture using a 2D Gaussian function.
    Gaussian(GaussianShape, ApertureType),
    /// a stack of an arbitrary number of the above apertures. The transmission factor at a given point is the
    /// product of all indiviual aperture on the stack (subtractive apodization).
    Stack(StackConfig, ApertureType),
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
            Self::BinaryCircle(circle, _) => circle.apodize(point),
            Self::BinaryRectangle(rectangle, _) => rectangle.apodize(point),
            Self::BinaryPolygon(p, _) => p.apodize(point),
            Self::Gaussian(g, _) => g.apodize(point),
            Self::Stack(s, _) => s.apodize(point),
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
// Ein Trait, der das grundlegende Verhalten einer Form beschreibt
pub trait Shape {
    /// Berechnet den Transmissionsfaktor (immer als "Loch" interpretiert).
    fn transmission_factor(&self, point: &Point2<Length>) -> f64;
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
                Self::BinaryCircle(conf,_) => Some(stack::plot_circle(conf)),
                Self::BinaryRectangle(conf,_ ) => {
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
                Self::BinaryPolygon(conf,_) => {
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
                Self::Gaussian(conf,_) => {
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
                Self::Stack(conf,_) => {
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
        assert!(matches!(Aperture::default(), Aperture::None));
    }
    #[test]
    fn binary_circle() {
        let c = CircleShape::new(meter!(1.0), meter!(1.0, 1.0)).unwrap();
        let ap = Aperture::BinaryCircle(c, ApertureType::Hole);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 1.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 0.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 2.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(2.0, 1.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 1.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 0.0)), 0.0);
        assert_eq!(ap.apodization_factor(&meter!(2.0, 2.0)), 0.0);
        let mut c = CircleShape::new(meter!(1.0), meter!(1.0, 1.0)).unwrap();
        c.set_aperture_type(ApertureType::Obstruction);
        let ap = Aperture::BinaryCircle(c, ApertureType::Obstruction);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 1.0)), 0.0);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 0.0)), 1.0);
    }
    #[test]
    fn binary_rectangle() {
        let r = RectangleShape::new(meter!(1.0), meter!(2.0), meter!(1.0, 1.0)).unwrap();
        let ap = Aperture::BinaryRectangle(r, ApertureType::Hole);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 1.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(1.5, 1.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(1.5, 2.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(0.5, 2.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(0.5, 0.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 0.0)), 0.0);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 2.1)), 0.0);
        let mut r = RectangleShape::new(meter!(1.0), meter!(2.0), meter!(1.0, 1.0)).unwrap();
        r.set_aperture_type(ApertureType::Obstruction);
        let ap = Aperture::BinaryRectangle(r, ApertureType::Obstruction);
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
        let ap = Aperture::BinaryPolygon(poly, ApertureType::Hole);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 0.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(2.0, 0.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 1.0)), 1.0);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 0.0)), 0.0);
        assert_eq!(ap.apodization_factor(&meter!(2.0, 1.0)), 0.0);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 1.0)), 0.0);
        let mut poly =
            PolygonConfig::new(vec![meter!(0.0, 0.0), meter!(2.0, 0.0), meter!(1.0, 1.0)]).unwrap();
        poly.set_aperture_type(ApertureType::Obstruction);
        let ap = Aperture::BinaryPolygon(poly, ApertureType::Obstruction);
        assert_eq!(ap.apodization_factor(&meter!(0.0, 0.0)), 0.0);
        assert_eq!(ap.apodization_factor(&meter!(2.0, 1.0)), 1.0);
    }
    #[test]
    fn gaussian() {
        let g = GaussianShape::new((meter!(1.0), meter!(1.0)), meter!(1.0, 1.0)).unwrap();
        let ap = Aperture::Gaussian(g, ApertureType::Hole);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 1.0)), 1.0);
        assert_eq!(
            ap.apodization_factor(&meter!(0.0, 0.0)),
            1.0 / 1.0_f64.exp()
        );
        let mut g = GaussianShape::new((meter!(1.0), meter!(1.0)), meter!(1.0, 1.0)).unwrap();
        g.set_aperture_type(ApertureType::Obstruction);
        let ap = Aperture::Gaussian(g, ApertureType::Obstruction);
        assert_eq!(ap.apodization_factor(&meter!(1.0, 1.0)), 0.0);
        assert_eq!(
            ap.apodization_factor(&meter!(0.0, 0.0)),
            1.0 - 1.0 / 1.0_f64.exp()
        );
    }
    #[test]
    fn stack() {
        let r = RectangleShape::new(meter!(1.0), meter!(1.0), meter!(0.5, 0.5)).unwrap();
        let r_ap = Aperture::BinaryRectangle(r, ApertureType::Hole);
        let c = CircleShape::new(meter!(1.0), meter!(0.0, 0.0)).unwrap();
        let c_ap = Aperture::BinaryCircle(c, ApertureType::Hole);
        let s = StackConfig::new(vec![r_ap, c_ap]);
        let s_ap = Aperture::Stack(s,ApertureType::Hole);
        assert_eq!(s_ap.apodization_factor(&meter!(0.0, 0.0)), 1.0);
        assert_eq!(s_ap.apodization_factor(&meter!(1.0, 0.0)), 1.0);
        assert_eq!(s_ap.apodization_factor(&meter!(0.0, 1.0)), 1.0);
        assert_eq!(s_ap.apodization_factor(&meter!(1.0, 1.0)), 0.0);
        assert_eq!(s_ap.apodization_factor(&meter!(-1.0, 0.0)), 0.0);
        assert_eq!(s_ap.apodization_factor(&meter!(0.0, -1.0)), 0.0);
        let r = RectangleShape::new(meter!(1.0), meter!(1.0), meter!(0.5, 0.5)).unwrap();
        let r_ap = Aperture::BinaryRectangle(r, ApertureType::Hole);
        let c = CircleShape::new(meter!(1.0), meter!(0.0, 0.0)).unwrap();
        let c_ap = Aperture::BinaryCircle(c, ApertureType::Hole);
        let mut s = StackConfig::new(vec![r_ap, c_ap]);
        s.set_aperture_type(ApertureType::Obstruction);
        let s_ap = Aperture::Stack(s, ApertureType::Obstruction);
        assert_eq!(s_ap.apodization_factor(&meter!(0.0, 0.0)), 0.0);
        assert_eq!(s_ap.apodization_factor(&meter!(1.0, 1.0)), 1.0);
    }
}
