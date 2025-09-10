use nalgebra::{Matrix2xX, Point2};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::millimeter};

use crate::{
    plottable::{PlotData, PlotSeries},
    utils::math_distribution_functions::ellipse,
};

use super::{Aperture, ApertureType, Apodize, CircleShape};

/// Configuration of an aperture stack
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    /// Returns a reference to the apertures of this [`StackConfig`].
    #[must_use]
    pub fn apertures(&self) -> &[Aperture] {
        &self.apertures
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
pub fn plot_circle(conf: &CircleShape) -> Vec<PlotSeries> {
    let circle_points = ellipse(
        (
            conf.center().x.get::<millimeter>(),
            conf.center().y.get::<millimeter>(),
        ),
        (
            conf.radius().get::<millimeter>(),
            conf.radius().get::<millimeter>(),
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
