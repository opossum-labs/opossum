#![warn(missing_docs)]
//! The shape of a generalized 2D super-Gaussian, on its own.
//!
//! A super-Gaussian is used for two quite different things in this crate: to spread a given energy
//! over a set of points
//! ([`General2DGaussian`](crate::distributions::energy::general_gaussian::General2DGaussian)), and
//! to give a pumped medium a transversal profile
//! ([`PumpSource`](crate::gain::PumpSource)). What they have in common is only the *shape* — where
//! it sits, how wide it is, how steep its flanks are and how it is turned — while what that shape is
//! then multiplied by differs.
//!
//! [`SuperGaussianShape`] is that common part, so that the description of a super-Gaussian exists
//! once rather than once per user. It carries no amplitude at all: it is **peak-normalised**, i.e.
//! [`SuperGaussianShape::value_at`] answers 1 on the axis and falls off from there. Whoever asks
//! decides what that 1 is worth.

use crate::{
    error::OpmResult,
    generic_validators::ValidateTrait,
    millimeter,
    types::validated_type_definitions::{
        ValidatedAngle1D, ValidatedCenter2D, ValidatedGaussianPower, ValidatedSideLengths2D,
    },
    utils::math_distribution_functions::{
        general_2d_super_gaussian_point_elliptical, general_2d_super_gaussian_point_rectangular,
    },
};
use nalgebra::Point2;
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::{
    angle::radian,
    f64::{Angle, Length},
};
use utoipa::ToSchema;

/// Deserialization shim for [`SuperGaussianShape`].
///
/// It lets a shape read from a file run through the very same validation as one built through
/// [`SuperGaussianShape::new`]. Without it there would be none at all: the validated types the
/// fields are held in are `#[serde(transparent)]`, so deserializing straight into them wraps
/// whatever the file says without ever looking at it, and a standard deviation of zero would then
/// divide by zero on the first evaluation.
#[derive(Deserialize)]
struct NonValidatedSuperGaussianShape {
    mu_xy: Point2<Length>,
    sigma_xy: Point2<Length>,
    power: f64,
    theta: Angle,
    rectangular: bool,
}
impl TryFrom<NonValidatedSuperGaussianShape> for SuperGaussianShape {
    type Error = String;
    fn try_from(helper: NonValidatedSuperGaussianShape) -> Result<Self, Self::Error> {
        Self::new(
            helper.mu_xy,
            helper.sigma_xy,
            helper.power,
            helper.theta,
            helper.rectangular,
        )
        .map_err(|e| e.to_string())
    }
}

/// The shape of a generalized 2D super-Gaussian, without an amplitude.
///
/// See the [module documentation](self) for why this is a type of its own.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, ToSchema, EnsureValidated)]
#[serde(try_from = "NonValidatedSuperGaussianShape")]
pub struct SuperGaussianShape {
    /// center of the distribution, i.e. where its peak sits
    #[schema(value_type = Object)]
    mu_xy: ValidatedCenter2D,
    /// standard deviation along each axis; unequal values make the distribution elliptical
    #[schema(value_type = Object)]
    sigma_xy: ValidatedSideLengths2D,
    /// steepness of the flanks: 1 is an ordinary Gaussian, larger values are super-Gaussians
    #[schema(value_type = f64)]
    power: ValidatedGaussianPower,
    /// rotation of the distribution in the transversal plane, counter-clockwise for positive values
    #[schema(value_type = f64)]
    theta: ValidatedAngle1D,
    /// whether the flanks are shaped rectangularly rather than elliptically; the two coincide for a
    /// power of 1
    #[validate(skip)]
    rectangular: bool,
}

impl Default for SuperGaussianShape {
    /// A round, unrotated, ordinary Gaussian of 5 mm standard deviation, centered on the axis.
    ///
    /// The same starting point
    /// [`General2DGaussian`](crate::distributions::energy::general_gaussian::General2DGaussian)
    /// has always used.
    fn default() -> Self {
        Self {
            mu_xy: ValidatedCenter2D::default(),
            sigma_xy: ValidatedSideLengths2D::try_new(millimeter!(5., 5.))
                .expect("5 mm is a valid standard deviation"),
            power: ValidatedGaussianPower::default(),
            theta: ValidatedAngle1D::default(),
            rectangular: false,
        }
    }
}

impl SuperGaussianShape {
    /// Create a new [`SuperGaussianShape`].
    ///
    /// # Arguments
    ///
    /// * `mu_xy` - the center the distribution peaks at.
    /// * `sigma_xy` - the standard deviation along each axis; unequal values make it elliptical.
    /// * `power` - steepness of the flanks. 1 is an ordinary Gaussian, larger values are
    ///   super-Gaussians approaching a flat top.
    /// * `theta` - rotation in the transversal plane, counter-clockwise for positive values.
    /// * `rectangular` - shape the flanks rectangularly instead of elliptically. The two coincide
    ///   for a power of 1.
    ///
    /// # Errors
    ///
    /// This function returns an error if the center or the rotation is not finite, if a standard
    /// deviation is not finite, zero or negative, or if the power is not finite, zero or negative.
    pub fn new(
        mu_xy: Point2<Length>,
        sigma_xy: Point2<Length>,
        power: f64,
        theta: Angle,
        rectangular: bool,
    ) -> OpmResult<Self> {
        Ok(Self {
            mu_xy: ValidatedCenter2D::try_new(mu_xy)?,
            sigma_xy: ValidatedSideLengths2D::try_new(sigma_xy)?,
            power: ValidatedGaussianPower::try_new(power)?,
            theta: ValidatedAngle1D::try_new(theta)?,
            rectangular,
        })
    }
    /// Return the center this shape peaks at.
    #[must_use]
    pub const fn center(&self) -> Point2<Length> {
        *self.mu_xy.get()
    }
    /// Return the standard deviation along each axis.
    #[must_use]
    pub const fn sigma(&self) -> Point2<Length> {
        *self.sigma_xy.get()
    }
    /// Return the steepness of the flanks: 1 is an ordinary Gaussian, larger values are
    /// super-Gaussians.
    #[must_use]
    pub const fn power(&self) -> f64 {
        *self.power.get()
    }
    /// Return the rotation of this shape in the transversal plane.
    #[must_use]
    pub const fn theta(&self) -> Angle {
        *self.theta.get()
    }
    /// Return whether the flanks are shaped rectangularly rather than elliptically.
    #[must_use]
    pub const fn rectangular(&self) -> bool {
        self.rectangular
    }
    /// Return the value of this shape at the given transversal position.
    ///
    /// The shape is **peak-normalised**: this answers 1 at its center and falls off towards the
    /// rim, never exceeding 1. It carries no amplitude of its own, so a caller multiplies the result
    /// by whatever the peak is worth to it — an energy, or the gain coefficient of a pumped medium.
    ///
    /// # Arguments
    ///
    /// * `point` - the transversal position to evaluate the shape at.
    ///
    /// # Returns
    ///
    /// The value of the shape there, in `0.0..=1.0`.
    #[must_use]
    pub fn value_at(&self, point: &Point2<Length>) -> f64 {
        let (sin_theta, cos_theta) = self.theta.get().get::<radian>().sin_cos();
        let position = Point2::new(point.x.value, point.y.value);
        let mu_xy = Point2::new(self.mu_xy.get().x.value, self.mu_xy.get().y.value);
        let sigma_xy = Point2::new(self.sigma_xy.get().x.value, self.sigma_xy.get().y.value);
        let power = *self.power.get();
        if self.rectangular {
            general_2d_super_gaussian_point_rectangular(
                &position, mu_xy, sigma_xy, power, sin_theta, cos_theta,
            )
        } else {
            general_2d_super_gaussian_point_elliptical(
                &position, mu_xy, sigma_xy, power, sin_theta, cos_theta,
            )
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::degree;
    use approx::assert_relative_eq;

    /// A round Gaussian of 1 mm standard deviation, centered on the axis.
    fn round_gaussian() -> OpmResult<SuperGaussianShape> {
        SuperGaussianShape::new(
            millimeter!(0., 0.),
            millimeter!(1., 1.),
            1.,
            degree!(0.),
            false,
        )
    }
    #[test]
    fn a_shape_peaks_at_its_center() -> OpmResult<()> {
        assert_relative_eq!(round_gaussian()?.value_at(&millimeter!(0., 0.)), 1.0);
        // ... and nowhere higher, wherever that center has been moved to
        let decentred = SuperGaussianShape::new(
            millimeter!(2., -3.),
            millimeter!(1., 1.),
            1.,
            degree!(0.),
            false,
        )?;
        assert_relative_eq!(decentred.value_at(&millimeter!(2., -3.)), 1.0);
        assert!(decentred.value_at(&millimeter!(0., 0.)) < 1.0);
        Ok(())
    }
    #[test]
    fn an_ordinary_gaussian_falls_to_exp_of_minus_a_half_at_one_sigma() -> OpmResult<()> {
        let shape = round_gaussian()?;
        for one_sigma in [
            millimeter!(1., 0.),
            millimeter!(-1., 0.),
            millimeter!(0., 1.),
            millimeter!(0., -1.),
        ] {
            assert_relative_eq!(shape.value_at(&one_sigma), f64::exp(-0.5));
        }
        Ok(())
    }
    #[test]
    fn each_axis_has_its_own_width() -> OpmResult<()> {
        // An elliptical shape falls off faster along the narrower axis, so the same offset is worth
        // a different value on each.
        let shape = SuperGaussianShape::new(
            millimeter!(0., 0.),
            millimeter!(1., 2.),
            1.,
            degree!(0.),
            false,
        )?;
        assert_relative_eq!(shape.value_at(&millimeter!(2., 0.)), f64::exp(-2.0));
        assert_relative_eq!(shape.value_at(&millimeter!(0., 2.)), f64::exp(-0.5));
        Ok(())
    }
    #[test]
    fn rotating_turns_the_ellipse_with_it() -> OpmResult<()> {
        // Turning an elliptical shape by a quarter turn swaps which axis is the narrow one, so the
        // two probe points trade their values.
        let upright = SuperGaussianShape::new(
            millimeter!(0., 0.),
            millimeter!(1., 2.),
            1.,
            degree!(0.),
            false,
        )?;
        let turned = SuperGaussianShape::new(
            millimeter!(0., 0.),
            millimeter!(1., 2.),
            1.,
            degree!(90.),
            false,
        )?;
        assert_relative_eq!(
            turned.value_at(&millimeter!(2., 0.)),
            upright.value_at(&millimeter!(0., 2.))
        );
        assert_relative_eq!(
            turned.value_at(&millimeter!(0., 2.)),
            upright.value_at(&millimeter!(2., 0.))
        );
        Ok(())
    }
    #[test]
    fn a_higher_power_flattens_the_top_and_steepens_the_flanks() -> OpmResult<()> {
        let gaussian = round_gaussian()?;
        let super_gaussian = SuperGaussianShape::new(
            millimeter!(0., 0.),
            millimeter!(1., 1.),
            4.,
            degree!(0.),
            false,
        )?;
        // well inside one sigma the super-Gaussian is still near its peak ...
        assert!(
            super_gaussian.value_at(&millimeter!(0.5, 0.))
                > gaussian.value_at(&millimeter!(0.5, 0.))
        );
        assert!(super_gaussian.value_at(&millimeter!(0.5, 0.)) > 0.99);
        // ... and beyond it, it has dropped off far more sharply
        assert!(
            super_gaussian.value_at(&millimeter!(2., 0.)) < gaussian.value_at(&millimeter!(2., 0.))
        );
        Ok(())
    }
    #[test]
    fn the_rectangular_flag_only_matters_beyond_a_power_of_one() -> OpmResult<()> {
        let corner = millimeter!(0.8, 0.4);
        let sigma = millimeter!(1., 0.5);
        for (power, differ) in [(1., false), (2., true)] {
            let elliptical =
                SuperGaussianShape::new(millimeter!(0., 0.), sigma, power, degree!(0.), false)?;
            let rectangular =
                SuperGaussianShape::new(millimeter!(0., 0.), sigma, power, degree!(0.), true)?;
            assert_eq!(
                (elliptical.value_at(&corner) - rectangular.value_at(&corner)).abs() > 1e-5,
                differ,
                "a power of {power} should{} tell the two apart",
                if differ { "" } else { " not" }
            );
        }
        Ok(())
    }
    #[test]
    fn a_shape_that_is_not_a_shape_is_refused() {
        let ok = millimeter!(1., 1.);
        // a standard deviation has to be a real, positive width ...
        for sigma in [
            millimeter!(0., 1.),
            millimeter!(1., 0.),
            millimeter!(-1., 1.),
            millimeter!(f64::NAN, 1.),
            millimeter!(1., f64::INFINITY),
        ] {
            assert!(
                SuperGaussianShape::new(millimeter!(0., 0.), sigma, 1., degree!(0.), false)
                    .is_err()
            );
        }
        // ... the power a positive number ...
        for power in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(
                SuperGaussianShape::new(millimeter!(0., 0.), ok, power, degree!(0.), false)
                    .is_err()
            );
        }
        // ... and center and rotation at least finite.
        assert!(
            SuperGaussianShape::new(millimeter!(f64::NAN, 0.), ok, 1., degree!(0.), false).is_err()
        );
        assert!(
            SuperGaussianShape::new(millimeter!(0., 0.), ok, 1., degree!(f64::NAN), false).is_err()
        );
        // a decentred, rotated, elliptical super-Gaussian is perfectly fine though
        assert!(
            SuperGaussianShape::new(
                millimeter!(-10., 3.),
                millimeter!(1., 2.),
                3.,
                degree!(30.),
                true
            )
            .is_ok()
        );
    }
    #[test]
    fn a_shape_read_from_a_file_is_validated_too() -> OpmResult<()> {
        let shape = SuperGaussianShape::default();
        let serialized =
            ron::to_string(&shape).map_err(|e| crate::error::OpossumError::Other(e.to_string()))?;
        let deserialized: SuperGaussianShape = ron::from_str(&serialized)
            .map_err(|e| crate::error::OpossumError::Other(e.to_string()))?;
        assert_eq!(shape, deserialized);
        // A hand-edited width of zero would divide by zero on the first evaluation, so it has to be
        // refused on the way in rather than on the way out. The accepted form stands next to it so
        // that the rejection is known to come from the value and not from a shape `ron` could not
        // read at all - the two differ in nothing else.
        let readable =
            "(mu_xy:(0.0,0.0),sigma_xy:(0.005,0.005),power:1.0,theta:0.0,rectangular:false)";
        let zero_width =
            "(mu_xy:(0.0,0.0),sigma_xy:(0.0,0.005),power:1.0,theta:0.0,rectangular:false)";
        assert!(ron::from_str::<SuperGaussianShape>(readable).is_ok());
        assert!(ron::from_str::<SuperGaussianShape>(zero_width).is_err());
        Ok(())
    }
    #[test]
    fn the_default_is_a_round_millimetre_scale_gaussian() {
        let shape = SuperGaussianShape::default();
        assert_relative_eq!(shape.value_at(&millimeter!(0., 0.)), 1.0);
        assert_relative_eq!(shape.value_at(&millimeter!(5., 0.)), f64::exp(-0.5));
        assert_relative_eq!(shape.value_at(&millimeter!(0., 5.)), f64::exp(-0.5));
    }
}
