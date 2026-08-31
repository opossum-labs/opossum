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
//! let ap = Aperture::new_circle(millimeter!(1.0), ApertureType::Hole, Some(millimeter!(1.0, 1.0))).unwrap();
//! assert_eq!(ap.apodize(&millimeter!(1.0,1.0,0.0)), 1.0);
//! assert_eq!(ap.apodize(&millimeter!(0.0,0.0,0.0)), 0.0);
//! ```
//! Furthermore, each aperture can act as a "hole" or as an "obstruction".
//! ```rust
//! use opossum_core::prelude::*;
//!
//! let ap = Aperture::new_circle(millimeter!(1.0), ApertureType::Obstruction, Some(millimeter!(1.0, 1.0))).unwrap();
//! assert_eq!(ap.apodize(&millimeter!(1.0, 1.0, 0.0)), 0.0);
//! assert_eq!(ap.apodize(&millimeter!(0.0, 0.0, 0.0)), 1.0);
//! ```
mod circle;
mod gaussian;
mod polygon;
mod rectangle;
mod stack;

use crate::{
    degree,
    error::{OpmResult, OpossumError},
    generic_validators::Validate,
    meter,
    prelude::Isometry,
    properties::Proptype,
    reporting::plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    types::validated_type_definitions::{ValidatedAngle1D, ValidatedCenter2D},
    utils::{default_from_name::DefaultFromName, math_distribution_functions::ellipse},
};
use core::f64;
use nalgebra::{Matrix2xX, MatrixXx2, Point2, Point3};
use num::Zero;
use opm_macros_lib::EnsureValidated;
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::{EnumIter, IntoEnumIterator};
use uom::si::{
    f64::{Angle, Length},
    length::millimeter,
};

pub use circle::CircleShape;
pub use gaussian::GaussianShape;
pub use polygon::PolygonShape;
pub use rectangle::RectangleShape;
pub use stack::StackShape;
use utoipa::ToSchema;

/// The apodization type of an [`Aperture`].
///
/// Each aperture can act as a "hole" or "obstruction"
#[derive(
    Copy,
    Default,
    Debug,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    ToSchema,
    EnumIter,
    EnsureValidated,
)]
pub enum ApertureType {
    /// the [`Aperture`] shape acts as a hole. The inner part of the shape is transparent.
    #[default]
    Hole,
    /// the [`Aperture`] shape represents an obstruction. The inner part of the shape is opaque.
    Obstruction,
}

impl DefaultFromName for ApertureType {}

impl Display for ApertureType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hole => write!(f, "Hole"),
            Self::Obstruction => write!(f, "Obstruction"),
        }
    }
}

/// An [`Aperture`] defines the shape and type of an optical aperture. The shape is defined by the enum [`ApertureShape`] and the type is defined by the enum [`ApertureType`].
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema, EnsureValidated)]
pub struct Aperture {
    shape: ApertureShape,
    a_type: ApertureType,
    #[validate(skip)]
    #[schema(value_type = Object)]
    #[serde(skip_serializing_if = "Option::is_none")]
    isometry: Option<Isometry>,
}

impl Aperture {
    /// Create a new [`Aperture`] by a given shape, type, center shift and rotation.
    /// The center shift and rotation are optional and default to no shift and no rotation.
    /// # Errors
    /// This function will return an error if the parameters of the aperture creation are invalid, e.g. if the center shift is indefinite or the rotation is `NaN` or `Infinity`.
    pub fn new(
        shape: ApertureShape,
        aperture_type: ApertureType,
        center_shift: Option<Point2<Length>>,
        rotation: Option<Angle>,
    ) -> OpmResult<Self> {
        // Check if the center shift is provided and differs from the origin
        let has_effective_shift = center_shift.is_some_and(|p| p != Point2::origin());

        // Check if the rotation is provided and differs from zero
        let has_effective_rotation = rotation.is_some_and(|r| r != Angle::zero());

        // Only construct an isometry if we actually deviate from the identity state
        let isometry = if has_effective_shift || has_effective_rotation {
            let validated_center =
                ValidatedCenter2D::try_new(center_shift.unwrap_or_else(Point2::origin))?;
            let validated_rotation =
                ValidatedAngle1D::try_new(rotation.unwrap_or_else(Angle::zero))?;

            let iso = Isometry::new(
                Point3::new(
                    validated_center.get().x,
                    validated_center.get().y,
                    meter!(0.0),
                ),
                Point3::new(degree!(0.0), degree!(0.0), *validated_rotation.get()),
            )?;

            Some(iso)
        } else {
            // No transformation needed; isometry remains None
            None
        };

        Ok(Self {
            shape,
            a_type: aperture_type,
            isometry,
        })
    }

    /// Calculate the transmission factor of a given point on this aperture. The value is in the range (0.0..=1.0)
    /// 0.0 is fully opaque, 1.0 fully transparent.
    /// The transmission factor is calculated by the [`apodize`](Aperture::apodize()) function of the respective shape. If the aperture type is `Obstruction`, the transmission factor is inverted.
    /// The point is transformed by the inverse of the isometry of the aperture before the apodization is calculated.
    /// Hole aperture: transmission factor is the same as the apodization of the shape.
    /// Obstruction aperture: transmission factor is 1.0 - apodization of the shape.
    #[must_use]
    pub fn apodize(&self, point: &Point3<Length>) -> f64 {
        // If isometry is present, transform the point; otherwise, use the point as-is
        let base_transmission = self.isometry.as_ref().map_or_else(
            || self.shape.apodize(point),
            |iso| {
                let transformed_point = iso.inverse_transform_point(point);
                self.shape.apodize(&transformed_point)
            },
        );

        if matches!(self.a_type, ApertureType::Obstruction) {
            1.0 - base_transmission
        } else {
            base_transmission
        }
    }

    /// Returns a reference to the shape of this [`Aperture`].
    #[must_use]
    pub const fn shape(&self) -> &ApertureShape {
        &self.shape
    }
    /// Returns a reference to the type of this [`Aperture`].
    #[must_use]
    pub const fn aperture_type(&self) -> &ApertureType {
        &self.a_type
    }
    /// Returns an optional reference to the isometry of this [`Aperture`].
    #[must_use]
    pub const fn isometry(&self) -> Option<&Isometry> {
        self.isometry.as_ref()
    }
    /// Return whether this [`Aperture`] delimits a region geometrically.
    ///
    /// An aperture describes how much light a surface transmits where, which is more general than
    /// an outline: only a binary shape used as a [`ApertureType::Hole`] encloses a well-defined
    /// region. A [`ApertureShape::Gaussian`] has no edge at all — it attenuates everywhere and
    /// transmits nowhere completely — an [`ApertureType::Obstruction`] delimits the *complement* of
    /// a region, and a [`ApertureShape::Stack`] may combine both. Anything that has to know where a
    /// body ends, rather than how much light passes, has to ask this first.
    ///
    /// # Returns
    ///
    /// `true` if the aperture can be read as the outline of a region.
    #[must_use]
    pub const fn is_geometric_bound(&self) -> bool {
        matches!(self.a_type, ApertureType::Hole) && self.shape.is_binary()
    }
    /// Set the shape of this [`Aperture`].
    pub fn set_shape(&mut self, shape: ApertureShape) {
        self.shape = shape;
    }
    /// Set the type of this [`Aperture`].
    pub const fn set_aperture_type(&mut self, aperture_type: ApertureType) {
        self.a_type = aperture_type;
    }

    /// Set the isometry of this [`Aperture`].
    /// It will only store the isometry if it deviates from the identity transformation.
    pub fn set_isometry(&mut self, iso: Isometry) {
        // Note: If your library supports .is_identity(), use: if iso.is_identity()
        if iso == Isometry::identity() {
            self.isometry = None;
        } else {
            self.isometry = Some(iso);
        }
    }

    /// Create a new circular aperture.
    ///
    /// # Errors
    ///
    /// This function will return an error if the parameters of the aperture creation are invalid.
    pub fn new_circle(
        radius: Length,
        aperture_type: ApertureType,
        translation: Option<Point2<Length>>,
    ) -> OpmResult<Self> {
        let config = CircleShape::new(radius)?;
        Self::new(
            ApertureShape::BinaryCircle(config),
            aperture_type,
            translation,
            None,
        )
    }
    /// Create a new retangular aperture.
    ///
    /// # Errors
    ///
    /// This function will return an error if the parameters of the aperture creation are invalid.
    pub fn new_rectangle(
        width: Length,
        height: Length,
        aperture_type: ApertureType,
        translation: Option<Point2<Length>>,
        rotation: Option<Angle>,
    ) -> OpmResult<Self> {
        let config = RectangleShape::new(width, height)?;
        Self::new(
            ApertureShape::BinaryRectangle(config),
            aperture_type,
            translation,
            rotation,
        )
    }
    /// Create a new Gaussian aperture.
    ///
    /// # Errors
    ///
    /// This function will return an error if the parameters of the aperture creation are invalid.
    pub fn new_gaussian(
        sigma: (Length, Length),
        aperture_type: ApertureType,
        translation: Option<Point2<Length>>,
        rotation: Option<Angle>,
    ) -> OpmResult<Self> {
        let config = GaussianShape::new(sigma)?;
        Self::new(
            ApertureShape::Gaussian(config),
            aperture_type,
            translation,
            rotation,
        )
    }
    /// Create a new polygon aperture.
    ///
    /// # Errors
    ///
    /// This function will return an error if the parameters of the aperture creation are invalid.
    pub fn new_polygon(
        points: Vec<Point2<Length>>,
        aperture_type: ApertureType,
        translation: Option<Point2<Length>>,
        rotation: Option<Angle>,
    ) -> OpmResult<Self> {
        let config = PolygonShape::new(points)?;
        Self::new(
            ApertureShape::BinaryPolygon(config),
            aperture_type,
            translation,
            rotation,
        )
    }
    /// Create a new stack aperture.
    ///
    /// # Errors
    /// This function will return an error if the parameters of the aperture creation are invalid.
    pub fn new_stack(
        apertures: Vec<Self>,
        aperture_type: ApertureType,
        translation: Option<Point2<Length>>,
        rotation: Option<Angle>,
    ) -> OpmResult<Self> {
        let config = StackShape::new(apertures)?;
        Self::new(
            ApertureShape::Stack(config),
            aperture_type,
            translation,
            rotation,
        )
    }

    /// Check if the aperture-shape is [`ApertureShape::Open`]
    #[must_use]
    pub const fn is_none(&self) -> bool {
        self.shape.is_none()
    }
}

/// Validator accepting only those [`Aperture`]s that delimit a region.
///
/// See [`Aperture::is_geometric_bound`] for what that rules out and why.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeometricBound;

impl Validate<Aperture> for GeometricBound {
    fn validate(&self, value: &Aperture) -> OpmResult<()> {
        if value.is_geometric_bound() {
            Ok(())
        } else {
            Err(OpossumError::Other(format!(
                "a {} aperture of shape '{}' does not delimit a region",
                value.aperture_type(),
                value.shape()
            )))
        }
    }
}

/// Different aperture types
#[derive(
    Default, Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema, EnumIter, EnsureValidated,
)]
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
    BinaryPolygon(PolygonShape),
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
            Self::Open => write!(f, "Open"),
            Self::BinaryCircle(_) => write!(f, "Circle"),
            Self::BinaryRectangle(_) => write!(f, "Rectangle"),
            Self::BinaryPolygon(_) => write!(f, "Polygon"),
            Self::Gaussian(_) => write!(f, "Gaussian"),
            Self::Stack(_) => write!(f, "Stacked apertures"),
        }
    }
}

impl ApertureShape {
    #[must_use]
    /// Check if the aperture is [`ApertureShape::Open`]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::Open)
    }
    /// Check whether this shape has a hard edge, i.e. transmits either fully or not at all.
    ///
    /// # Returns
    ///
    /// `true` for the binary shapes, `false` for [`ApertureShape::Open`],
    /// [`ApertureShape::Gaussian`] and [`ApertureShape::Stack`].
    #[must_use]
    pub const fn is_binary(&self) -> bool {
        matches!(
            self,
            Self::BinaryCircle(_) | Self::BinaryRectangle(_) | Self::BinaryPolygon(_)
        )
    }
    /// Return one instance of every shape that does **not** delimit a region.
    ///
    /// These are the shapes that cannot state where a medium ends, so anything describing an
    /// outline rather than a transmission mask has to refuse them — the clear aperture of a volume
    /// node ([`CLEAR_APERTURE`](crate::geometry::body::CLEAR_APERTURE)) above all, whose property is
    /// guarded by [`Validator::ApertureDelimitsRegion`](crate::properties::validator::Validator).
    /// A user interface offering a choice of shapes asks here rather than keeping a list of its own,
    /// which would silently go stale as soon as a variant is added.
    ///
    /// The list is derived from [`Self::is_binary`] rather than spelled out, so the two cannot
    /// disagree. The returned values are the [`Default`] of each variant: only *which* variant each
    /// one is carries meaning here, never its contents.
    ///
    /// # Returns
    ///
    /// One instance of [`Self::Open`], [`Self::Gaussian`] and [`Self::Stack`].
    #[must_use]
    pub fn non_delimiting() -> Vec<Self> {
        Self::iter().filter(|shape| !shape.is_binary()).collect()
    }
    /// Calculate the transmission factor of a given point on the [`Aperture`]. The value is in the range (0.0..=1.0)
    /// 0.0 is fully opaque, 1.0 fully transparent.
    #[must_use]
    fn apodize(&self, point: &Point3<Length>) -> f64 {
        // Resolve both transmission and type in a single match for clarity and correctness
        match self {
            Self::Open => 1.0,
            Self::BinaryCircle(shape) => shape.transmission_factor(point),
            Self::BinaryRectangle(shape) => shape.transmission_factor(point),
            Self::BinaryPolygon(shape) => shape.transmission_factor(point),
            Self::Gaussian(shape) => shape.transmission_factor(point),
            Self::Stack(apertures) => apertures
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

/// Trait for the calculation of the transmission factor for each shape.
pub trait Shape {
    /// Calculate the transmission factor (always treated as aperture type `hole`).
    fn transmission_factor(&self, point: &Point3<Length>) -> f64;
}

impl Plottable for Aperture {
    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let iso = self.isometry();
        let plt_series_opt = match plt_type {
            PlotType::Line2D(_) | PlotType::Scatter2D(_) => match &self.shape {
                ApertureShape::Open => None,
                ApertureShape::BinaryCircle(conf) => Some(stack::plot_circle(*conf, iso)?),
                ApertureShape::BinaryRectangle(conf) => {
                    let mut points = [
                        Point2::new(conf.width() / 2., conf.height() / 2.),
                        Point2::new(-conf.width() / 2., conf.height() / 2.),
                        Point2::new(-conf.width() / 2., -conf.height() / 2.),
                        Point2::new(conf.width() / 2., -conf.height() / 2.),
                    ];
                    for p in &mut points {
                        // Transform the point only if an isometry is present
                        if let Some(iso) = iso {
                            let transformed_p =
                                iso.transform_point(&Point3::new(p.x, p.y, meter!(0.0)));
                            p.x = transformed_p.x;
                            p.y = transformed_p.y;
                        }
                    }
                    let plt_data_points = points
                        .iter()
                        .flat_map(|p| vec![p.x.get::<millimeter>(), p.y.get::<millimeter>()])
                        .collect::<Vec<f64>>();
                    let plt_dat = PlotData::Dim2 {
                        xy_data: Matrix2xX::<f64>::from_vec(plt_data_points).transpose(),
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
                    // Extract translation values if isometry exists, otherwise default to 0.0
                    let (trans_x, trans_y) = iso.map_or((0.0, 0.0), |iso| {
                        (
                            iso.translation().x.get::<millimeter>(),
                            iso.translation().y.get::<millimeter>(),
                        )
                    });

                    let circle_points = ellipse(
                        (trans_x, trans_y),
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
    use crate::{meter, millimeter};
    #[test]
    fn default() {
        assert!(matches!(ApertureShape::default(), ApertureShape::Open));
    }
    /// Every shape is either able to delimit a region or listed as unable to — a new variant must
    /// end up on exactly one of the two sides, never silently on neither.
    #[test]
    fn non_delimiting_covers_every_non_binary_shape() {
        let non_delimiting = ApertureShape::non_delimiting();
        assert!(
            !non_delimiting.iter().any(ApertureShape::is_binary),
            "a shape that delimits a region must not be listed as one that does not"
        );
        assert_eq!(
            non_delimiting.len()
                + ApertureShape::iter()
                    .filter(ApertureShape::is_binary)
                    .count(),
            ApertureShape::iter().count(),
            "every shape has to be classified"
        );
        for shape in [
            ApertureShape::Open,
            ApertureShape::Gaussian(GaussianShape::default()),
            ApertureShape::Stack(StackShape::default()),
        ] {
            assert!(
                non_delimiting
                    .iter()
                    .any(|s| std::mem::discriminant(s) == std::mem::discriminant(&shape)),
                "{shape} has no edge to bound a medium with and must be listed"
            );
        }
    }
    #[test]
    fn test_new_circle() {
        let center = meter!(0.0, 0.0);
        // Valid radius
        assert!(Aperture::new_circle(meter!(1.0), ApertureType::Hole, Some(center)).is_ok());
        // Invalid radius (negative)
        assert!(Aperture::new_circle(meter!(-1.0), ApertureType::Hole, Some(center)).is_err());
    }
    #[test]
    fn test_new_rectangle() {
        let center = meter!(0.0, 0.0);
        // Valid dimensions
        assert!(
            Aperture::new_rectangle(
                meter!(1.0),
                meter!(1.0),
                ApertureType::Hole,
                Some(center),
                None
            )
            .is_ok()
        );
        // Invalid height
        assert!(
            Aperture::new_rectangle(
                meter!(1.0),
                meter!(0.0),
                ApertureType::Hole,
                Some(center),
                None
            )
            .is_err()
        );
    }

    #[test]
    fn test_new_gaussian() {
        let center = meter!(0.0, 0.0);
        // Valid sigma
        assert!(
            Aperture::new_gaussian(
                (meter!(1.0), meter!(1.0)),
                ApertureType::Hole,
                Some(center),
                None
            )
            .is_ok()
        );
        // Invalid sigma (zero)
        assert!(
            Aperture::new_gaussian(
                (meter!(0.0), meter!(1.0)),
                ApertureType::Hole,
                Some(center),
                None
            )
            .is_err()
        );
    }

    #[test]
    fn test_new_polygon() {
        // Valid triangle
        let points = vec![meter!(0.0, 0.0), meter!(1.0, 0.0), meter!(0.0, 1.0)];
        assert!(Aperture::new_polygon(points, ApertureType::Hole, None, None).is_ok());
        // Invalid polygon (too few points)
        let points_too_few = vec![meter!(0.0, 0.0), meter!(1.0, 0.0)];
        assert!(Aperture::new_polygon(points_too_few, ApertureType::Hole, None, None).is_err());
    }

    #[test]
    fn test_new_stack() -> OpmResult<()> {
        let circle = Aperture::new_circle(meter!(1.0), ApertureType::Hole, Some(meter!(0.0, 0.0)))?;
        let rect = Aperture::new_rectangle(
            meter!(1.0),
            meter!(1.0),
            ApertureType::Hole,
            Some(meter!(0.0, 0.0)),
            None,
        )?;

        // Stack returns Self directly, not OpmResult
        let stack = Aperture::new_stack(vec![circle, rect], ApertureType::Obstruction, None, None)?;

        if let ApertureShape::Stack(config) = stack.shape {
            assert_eq!(config.apertures().len(), 2);
            assert_eq!(stack.a_type, ApertureType::Obstruction);
        } else {
            panic!("Expected Aperture::Stack variant");
        }
        Ok(())
    }
    #[test]
    fn display_shape() {
        assert_eq!(ApertureShape::Open.to_string(), "Open");
        assert_eq!(
            ApertureShape::BinaryCircle(CircleShape::default()).to_string(),
            "Circle"
        );
        assert_eq!(
            ApertureShape::BinaryRectangle(RectangleShape::default()).to_string(),
            "Rectangle"
        );
        assert_eq!(
            ApertureShape::Gaussian(GaussianShape::default()).to_string(),
            "Gaussian"
        );
        assert_eq!(
            ApertureShape::BinaryPolygon(PolygonShape::default()).to_string(),
            "Polygon"
        );
        assert_eq!(
            ApertureShape::Stack(StackShape::default()).to_string(),
            "Stacked apertures"
        );
    }
    #[test]
    fn display_aperture_type() {
        assert_eq!(ApertureType::Hole.to_string(), "Hole");
        assert_eq!(ApertureType::Obstruction.to_string(), "Obstruction");
    }
    #[test]
    fn test_is_none() -> OpmResult<()> {
        assert!(ApertureShape::Open.is_none());
        let circle = Aperture::new_circle(meter!(1.0), ApertureType::Hole, Some(meter!(0.0, 0.0)))?;
        assert!(!circle.is_none());
        Ok(())
    }
    #[test]
    fn test_obstruction_logic() -> OpmResult<()> {
        // A circle as a hole (default)
        let hole = Aperture::new_circle(meter!(1.0), ApertureType::Hole, Some(meter!(0.0, 0.0)))?;
        // A circle as an obstruction
        let block = Aperture::new_circle(
            meter!(1.0),
            ApertureType::Obstruction,
            Some(meter!(0.0, 0.0)),
        )?;

        let p_inside = meter!(0.5, 0.0, 0.0);
        let p_outside = meter!(2.0, 0.0, 0.0);

        assert_eq!(hole.apodize(&p_inside), 1.0);
        assert_eq!(hole.apodize(&p_outside), 0.0);

        assert_eq!(block.apodize(&p_inside), 0.0);
        assert_eq!(block.apodize(&p_outside), 1.0);
        Ok(())
    }
    #[test]
    fn aperture_type() {
        let mut ap = Aperture::default();
        assert_eq!(ap.aperture_type(), &ApertureType::Hole);
        ap.set_aperture_type(ApertureType::Obstruction);
        assert_eq!(ap.aperture_type(), &ApertureType::Obstruction);
    }
    #[test]
    fn isometry() -> OpmResult<()> {
        let mut ap = Aperture::default();
        assert_eq!(ap.isometry(), None);
        ap.set_isometry(Isometry::identity());
        assert_eq!(ap.isometry(), None);
        ap.set_isometry(Isometry::new_along_z(millimeter!(1.0))?);
        assert_eq!(
            ap.isometry(),
            Some(&Isometry::new_along_z(millimeter!(1.0))?)
        );
        Ok(())
    }
}
