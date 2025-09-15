use super::{Aperture, Shape};
use crate::{
    apertures::CircleShape,
    plottable::{PlotData, PlotSeries},
    utils::math_distribution_functions::ellipse,
};
use nalgebra::{Matrix2xX, Point2};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::millimeter};
/// Configuration of an aperture stack
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StackShape {
    apertures: Vec<Aperture>,
}
impl StackShape {
    /// Creates a new [`StackShape`] by a given set of apertures.
    ///
    /// All aperture transmissions are multiplied, thus realizing a "subtractive" aperture. After that the transmission can be "inverted"
    /// (`transmission = 1.0 - transmission`) by setting the aperture type to [`ApertureType::Obstruction`].
    #[must_use]
    pub fn new(apertures: Vec<Aperture>) -> Self {
        Self { apertures }
    }
    /// Returns a reference to the apertures of this [`StackConfig`].
    #[must_use]
    pub fn apertures(&self) -> &[Aperture] {
        &self.apertures
    }
}
impl Shape for StackShape {
    fn transmission_factor(&self, point: &Point2<Length>) -> f64 {
        let mut transmission = 1.0;
        for a in &self.apertures {
            transmission *= a.apodize(point);
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
#[cfg(test)]
mod test {
    use super::super::{ApertureType, CircleShape, RectangleShape};
    use super::*;
    use crate::meter;

    #[test]
    fn stack() {
        let r = RectangleShape::new(meter!(1.0), meter!(1.0), meter!(0.5, 0.5)).unwrap();
        let r_ap = Aperture::BinaryRectangle(r, ApertureType::Hole);
        let c = CircleShape::new(meter!(1.0), meter!(0.0, 0.0)).unwrap();
        let c_ap = Aperture::BinaryCircle(c, ApertureType::Hole);
        let s = StackShape::new(vec![r_ap, c_ap]);
        let s_ap = Aperture::Stack(s, ApertureType::Hole);
        assert_eq!(s_ap.apodize(&meter!(0.0, 0.0)), 1.0);
        assert_eq!(s_ap.apodize(&meter!(1.0, 0.0)), 1.0);
        assert_eq!(s_ap.apodize(&meter!(0.0, 1.0)), 1.0);
        assert_eq!(s_ap.apodize(&meter!(1.0, 1.0)), 0.0);
        assert_eq!(s_ap.apodize(&meter!(-1.0, 0.0)), 0.0);
        assert_eq!(s_ap.apodize(&meter!(0.0, -1.0)), 0.0);
    }
}
