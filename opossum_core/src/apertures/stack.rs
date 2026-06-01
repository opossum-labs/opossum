use super::{Aperture, Shape};
use crate::{
    apertures::CircleShape,
    error::OpmResult,
    reporting::plottable::{PlotData, PlotSeries},
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
    /// All aperture transmissions are multiplied, thus realizing a "subtractive" aperture.
    #[must_use]
    pub const fn new(apertures: Vec<Aperture>) -> Self {
        Self { apertures }
    }
    /// Returns a reference to the apertures of this [`StackShape`].
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
pub fn plot_circle(conf: &CircleShape) -> OpmResult<Vec<PlotSeries>> {
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
    )?;
    let plt_dat = PlotData::Dim2 {
        xy_data: Matrix2xX::from_vec(
            circle_points
                .iter()
                .flat_map(|p| vec![p.x, p.y])
                .collect::<Vec<f64>>(),
        )
        .transpose(),
    };
    Ok(vec![PlotSeries::new(
        &plt_dat,
        RGBAColor(0, 0, 0, 1.),
        Some("Aperture".to_owned()),
    )])
}
#[cfg(test)]
mod test {
    use approx::assert_abs_diff_eq;

    use super::super::{ApertureType, CircleShape, RectangleShape};
    use super::*;
    use crate::error::OpmResult;
    use crate::meter;

    #[test]
    fn stack() -> OpmResult<()> {
        let r = RectangleShape::new(meter!(1.0), meter!(1.0), meter!(0.5, 0.5))?;
        let r_ap = Aperture::BinaryRectangle(r, ApertureType::Hole);
        let c = CircleShape::new(meter!(1.0), meter!(0.0, 0.0))?;
        let c_ap = Aperture::BinaryCircle(c, ApertureType::Hole);
        let s = StackShape::new(vec![r_ap, c_ap]);
        let s_ap = Aperture::Stack(s, ApertureType::Hole);
        assert_eq!(s_ap.apodize(&meter!(0.0, 0.0)), 1.0);
        assert_eq!(s_ap.apodize(&meter!(1.0, 0.0)), 1.0);
        assert_eq!(s_ap.apodize(&meter!(0.0, 1.0)), 1.0);
        assert_eq!(s_ap.apodize(&meter!(1.0, 1.0)), 0.0);
        assert_eq!(s_ap.apodize(&meter!(-1.0, 0.0)), 0.0);
        assert_eq!(s_ap.apodize(&meter!(0.0, -1.0)), 0.0);
        Ok(())
    }
    #[test]
    fn test_stack_transmission_factor() -> OpmResult<()> {
        // 1. Create a circle at (0,0) with radius 1.0
        let circle = CircleShape::new(meter!(1.0), meter!(0.0, 0.0))?;
        let circle_ap = Aperture::BinaryCircle(circle, ApertureType::Hole);

        // 2. Create a rectangle at (1,0) with width 2.0 and height 2.0
        // This rectangle covers x from 0.0 to 2.0 and y from -1.0 to 1.0
        let rect = RectangleShape::new(meter!(2.0), meter!(2.0), meter!(1.0, 0.0))?;
        let rect_ap = Aperture::BinaryRectangle(rect, ApertureType::Hole);

        // 3. Create the stack
        let stack = StackShape::new(vec![circle_ap, rect_ap]);

        // --- Test Points ---

        // Point (0.5, 0.0): Inside BOTH circle and rectangle
        // Expected: 1.0 * 1.0 = 1.0
        assert_abs_diff_eq!(
            stack.transmission_factor(&meter!(0.5, 0.0)),
            1.0,
            epsilon = 1e-12
        );

        // Point (-0.5, 0.0): Inside circle, but OUTSIDE rectangle (x < 0)
        // Expected: 1.0 * 0.0 = 0.0
        assert_abs_diff_eq!(
            stack.transmission_factor(&meter!(-0.5, 0.0)),
            0.0,
            epsilon = 1e-12
        );

        // Point (1.5, 0.0): Outside circle, but INSIDE rectangle
        // Expected: 0.0 * 1.0 = 0.0
        assert_abs_diff_eq!(
            stack.transmission_factor(&meter!(1.5, 0.0)),
            0.0,
            epsilon = 1e-12
        );

        // Point (5.0, 5.0): Outside BOTH
        // Expected: 0.0 * 0.0 = 0.0
        assert_abs_diff_eq!(
            stack.transmission_factor(&meter!(5.0, 5.0)),
            0.0,
            epsilon = 1e-12
        );
        Ok(())
    }
}
