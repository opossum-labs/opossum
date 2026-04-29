use super::Shape;
use crate::{
    apertures::{Aperture, CircleShape},
    error::OpmResult,
    generic_validators::{AllNotEmpty, Pass, ValidateTrait},
    prelude::Isometry,
    reporting::plottable::{PlotData, PlotSeries},
    utils::math_distribution_functions::ellipse,
    validated_vec, validated_vec_type,
};
use nalgebra::{Matrix2xX, Point3};
use opm_macros_lib::EnsureValidated;
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::millimeter};
use utoipa::ToSchema;

type ValidatedApertureStack = validated_vec_type!(Vec<Aperture>, Pass, AllNotEmpty);
type ValidatedApertureStack = validated_vec_type!(Vec<Aperture>, Pass, AllNotEmpty);
/// Configuration of an aperture stack
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnsureValidated, ToSchema)]
pub struct StackShape {
    #[schema(value_type = Object)]
    apertures: ValidatedApertureStack,
}

impl Default for StackShape {
impl Default for StackShape {
    fn default() -> Self {
        Self {
            apertures: validated_vec!(vec![Aperture::default()], Pass, AllNotEmpty).unwrap(),
        }
        Self {
            apertures: validated_vec!(vec![Aperture::default()], Pass, AllNotEmpty).unwrap(),
        }
    }
}
impl StackShape {
    /// Creates a new [`StackShape`] by a given set of apertures.
    ///
    /// All aperture transmissions are multiplied, thus realizing a "subtractive" aperture.
    /// # Errors
    /// This function will return an error if the given vector of apertures is empty.
    pub fn new(apertures: Vec<Aperture>) -> OpmResult<Self> {
        let mut stack = Self::default();
        stack.set_apertures(apertures)?;
        Ok(stack)
    }
    /// Returns a reference to the apertures of this [`StackShape`].
    #[must_use]
    pub fn apertures(&self) -> &[Aperture] {
        self.apertures.get()
    }

    /// Set the apertures of this [`StackShape`].
    ///
    /// All aperture transmissions are multiplied, thus realizing a "subtractive" aperture.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given vector of apertures is empty.
    fn set_apertures(&mut self, apertures: Vec<Aperture>) -> OpmResult<()> {
        self.apertures.set(apertures)?;
        Ok(())
    }

    /// Add an aperture to this [`StackShape`].
    /// # Errors
    /// This function will return an error if the aperture cannot be added, e.g. if the aperture is invalid.
    pub fn add_aperture(&mut self, aperture: Aperture) -> OpmResult<()> {
        self.apertures.push(aperture)?;
        Ok(())
    }

    /// Delete an aperture at a given index from this [`StackShape`].
    /// # Errors
    /// This function will return an error if the index is out of bounds.
    pub fn delete_aperture(&mut self, index: usize) -> OpmResult<()> {
        self.apertures.remove(index)?;
        Ok(())
    }

    /// Get a reference to an aperture at a given index from this [`StackShape`].
    /// # Errors
    /// This function will return an error if the index is out of bounds.
    pub fn get_aperture(&self, index: usize) -> OpmResult<&Aperture> {
        self.apertures.get_at_index(index)
    }

    /// Set an aperture at a given index in this [`StackShape`].
    /// # Errors
    /// This function will return an error if the index is out of bounds or if the aperture is invalid.
    pub fn set_aperture(&mut self, index: usize, aperture: Aperture) -> OpmResult<()> {
        self.apertures.replace(index, aperture)?;
        Ok(())
    }
}
impl Shape for StackShape {
    fn transmission_factor(&self, point: &Point3<Length>) -> f64 {
        let mut transmission = 1.0;
        for a in &self.apertures {
            transmission *= a.apodize(point);
        }
        transmission
    }
}
pub fn plot_circle(conf: CircleShape, isometry: &Isometry) -> Vec<PlotSeries> {
    let circle_points = ellipse(
        (
            isometry.translation().x.get::<millimeter>(),
            isometry.translation().y.get::<millimeter>(),
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
    use approx::assert_abs_diff_eq;

    use super::super::{Aperture, ApertureType, CircleShape, RectangleShape};
    use super::super::{Aperture, ApertureType, CircleShape, RectangleShape};
    use super::*;
    use crate::meter;
    use crate::prelude::ApertureShape;

    #[test]
    fn stack() {
        let r = RectangleShape::new(meter!(1.0), meter!(1.0)).unwrap();
        let r_ap = Aperture::new(
            ApertureShape::BinaryRectangle(r),
            ApertureType::Hole,
            Some(meter!(0.5, 0.5)),
            None,
        )
        .unwrap();
        let c = CircleShape::new(meter!(1.0)).unwrap();
        let c_ap = Aperture::new(
            ApertureShape::BinaryCircle(c),
            ApertureType::Hole,
            None,
            None,
        )
        .unwrap();
        let s = StackShape::new(vec![r_ap, c_ap]).unwrap();
        let s_ap = Aperture::new(ApertureShape::Stack(s), ApertureType::Hole, None, None).unwrap();
        assert_eq!(s_ap.apodize(&meter!(0.0, 0.0, 0.0)), 1.0);
        assert_eq!(s_ap.apodize(&meter!(1.0, 0.0, 0.0)), 1.0);
        assert_eq!(s_ap.apodize(&meter!(0.0, 1.0, 0.0)), 1.0);
        assert_eq!(s_ap.apodize(&meter!(1.0, 1.0, 0.0)), 0.0);
        assert_eq!(s_ap.apodize(&meter!(-1.0, 0.0, 0.0)), 0.0);
        assert_eq!(s_ap.apodize(&meter!(0.0, -1.0, 0.0)), 0.0);
    }
    #[test]
    fn test_stack_transmission_factor() {
        // 1. Create a circle at (0,0) with radius 1.0
        let circle = CircleShape::new(meter!(1.0)).unwrap();
        let circle_ap = ApertureShape::BinaryCircle(circle);
        let circle_ap = Aperture::new(circle_ap, ApertureType::Hole, None, None).unwrap();
        // 2. Create a rectangle at (1,0) with width 2.0 and height 2.0
        // This rectangle covers x from 0.0 to 2.0 and y from -1.0 to 1.0
        let rect = RectangleShape::new(meter!(2.0), meter!(2.0)).unwrap();
        let rect_ap = ApertureShape::BinaryRectangle(rect);
        let rect_ap =
            Aperture::new(rect_ap, ApertureType::Hole, Some(meter!(1.0, 0.0)), None).unwrap();
        // 3. Create the stack
        let stack = StackShape::new(vec![circle_ap, rect_ap]).unwrap();

        // --- Test Points ---

        // Point (0.5, 0.0): Inside BOTH circle and rectangle
        // Expected: 1.0 * 1.0 = 1.0
        assert_abs_diff_eq!(
            stack.transmission_factor(&meter!(0.5, 0.0, 0.0)),
            1.0,
            epsilon = 1e-12
        );

        // Point (-0.5, 0.0): Inside circle, but OUTSIDE rectangle (x < 0)
        // Expected: 1.0 * 0.0 = 0.0
        assert_abs_diff_eq!(
            stack.transmission_factor(&meter!(-0.5, 0.0, 0.0)),
            0.0,
            epsilon = 1e-12
        );

        // Point (1.5, 0.0): Outside circle, but INSIDE rectangle
        // Expected: 0.0 * 1.0 = 0.0
        assert_abs_diff_eq!(
            stack.transmission_factor(&meter!(1.5, 0.0, 0.0)),
            0.0,
            epsilon = 1e-12
        );

        // Point (5.0, 5.0): Outside BOTH
        // Expected: 0.0 * 0.0 = 0.0
        assert_abs_diff_eq!(
            stack.transmission_factor(&meter!(5.0, 5.0, 0.0)),
            0.0,
            epsilon = 1e-12
        );
    }
}
