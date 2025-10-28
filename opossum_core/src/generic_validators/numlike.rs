use num::Zero;
use std::fmt::Debug;
use uom::si::{Dimension, Quantity};

pub trait NumLike: Clone + PartialEq + Debug + PartialOrd {
    fn normal(&self) -> bool;
    fn finite(&self) -> bool;
    fn zero(&self) -> bool;
    fn nan(&self) -> bool;
    fn sign_positive(&self) -> bool;
    fn sign_negative(&self) -> bool;
    fn not_zero(&self) -> bool {
        !self.zero()
    }
    fn not_nan(&self) -> bool {
        !self.nan()
    }
    fn smaller_than(&self, other: &Self) -> bool;
}

impl NumLike for f64 {
    fn normal(&self) -> bool {
        self.is_normal()
    }

    fn finite(&self) -> bool {
        self.is_finite()
    }

    fn zero(&self) -> bool {
        self.is_zero()
    }

    fn nan(&self) -> bool {
        self.is_nan()
    }

    fn sign_positive(&self) -> bool {
        self.is_sign_positive()
    }

    fn sign_negative(&self) -> bool {
        self.is_sign_negative()
    }
    fn smaller_than(&self, other: &Self) -> bool {
        self < other
    }
}

impl NumLike for i32 {
    fn normal(&self) -> bool {
        *self != 0
    }

    fn finite(&self) -> bool {
        true
    }

    fn zero(&self) -> bool {
        self.is_zero()
    }

    fn nan(&self) -> bool {
        false
    }

    fn sign_positive(&self) -> bool {
        *self >= 0
    }

    fn sign_negative(&self) -> bool {
        *self < 0
    }
    fn smaller_than(&self, other: &Self) -> bool {
        self < other
    }
}

impl NumLike for usize {
    fn normal(&self) -> bool {
        *self != 0
    }

    fn finite(&self) -> bool {
        true
    }

    fn zero(&self) -> bool {
        self.is_zero()
    }

    fn nan(&self) -> bool {
        false
    }

    fn sign_positive(&self) -> bool {
        true
    }

    fn sign_negative(&self) -> bool {
        false
    }
    fn smaller_than(&self, other: &Self) -> bool {
        self < other
    }
}

impl<D, U> NumLike for Quantity<D, U, f64>
where
    D: Dimension + ?Sized,
    U: uom::si::Units<f64> + ?Sized,
{
    fn normal(&self) -> bool {
        self.value.is_normal()
    }

    fn finite(&self) -> bool {
        self.value.is_finite()
    }

    fn zero(&self) -> bool {
        self.value.is_zero()
    }

    fn nan(&self) -> bool {
        self.value.is_nan()
    }

    fn sign_positive(&self) -> bool {
        self.value.is_sign_positive()
    }

    fn sign_negative(&self) -> bool {
        self.value.is_sign_negative()
    }

    fn smaller_than(&self, other: &Self) -> bool {
        self.value < other.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::angle::radian;
    use uom::si::f64::{Angle, Length};
    use uom::si::length::meter;

    #[test]
    fn test_numlike_f64() {
        let finite = 1.0_f64;
        let zero = 0.0_f64;
        let nan = f64::NAN;
        let inf = f64::INFINITY;

        assert!(finite.normal());
        assert!(finite.finite());
        assert!(!finite.zero());
        assert!(!finite.nan());
        assert!(finite.sign_positive());
        assert!(!(-finite).sign_positive());
        assert!((-finite).sign_negative());

        assert!(!zero.normal());
        assert!(zero.finite());
        assert!(zero.zero());
        assert!(!zero.nan());
        assert!(zero.sign_positive());
        assert!(!zero.sign_negative());

        assert!(!nan.finite());
        assert!(nan.nan());
        assert!(!nan.normal());

        assert!(!inf.finite());
        assert!(!inf.zero());
        assert!(inf.sign_positive());
        assert!(!inf.sign_negative());
    }

    #[test]
    fn test_numlike_i32() {
        let pos = 42_i32;
        let neg = -42_i32;
        let zero = 0_i32;

        assert!(pos.normal());
        assert!(pos.finite());
        assert!(!pos.zero());
        assert!(!pos.nan());
        assert!(pos.sign_positive());
        assert!(!pos.sign_negative());

        assert!(!zero.normal());
        assert!(zero.finite());
        assert!(zero.zero());
        assert!(!zero.nan());
        assert!(zero.sign_positive());
        assert!(!zero.sign_negative());

        assert!(neg.normal());
        assert!(neg.finite());
        assert!(!neg.zero());
        assert!(!neg.nan());
        assert!(!neg.sign_positive());
        assert!(neg.sign_negative());
    }

    #[test]
    fn test_numlike_usize() {
        let pos = 42_usize;
        let zero = 0_usize;

        assert!(pos.normal());
        assert!(pos.finite());
        assert!(!pos.zero());
        assert!(!pos.nan());
        assert!(pos.sign_positive());
        assert!(!pos.sign_negative());

        assert!(!zero.normal());
        assert!(zero.finite());
        assert!(zero.zero());
        assert!(!zero.nan());
        assert!(zero.sign_positive());
        assert!(!zero.sign_negative());
    }

    #[test]
    fn test_numlike_quantity_length() {
        let length = Length::new::<meter>(1.0);
        let zero_length = Length::new::<meter>(0.0);
        let neg_length = Length::new::<meter>(-1.0);
        let inf_length = Length::new::<meter>(f64::INFINITY);
        let nan_length = Length::new::<meter>(f64::NAN);

        assert!(length.normal());
        assert!(length.finite());
        assert!(!length.zero());
        assert!(!length.nan());
        assert!(length.sign_positive());
        assert!(!neg_length.sign_positive());
        assert!(neg_length.sign_negative());

        assert!(!zero_length.normal());
        assert!(zero_length.finite());
        assert!(zero_length.zero());
        assert!(!zero_length.nan());

        assert!(!inf_length.finite());
        assert!(!inf_length.normal());
        assert!(!inf_length.zero());

        assert!(nan_length.nan());
        assert!(!nan_length.normal());
        assert!(!nan_length.finite());
    }

    #[test]
    fn test_numlike_quantity_angle() {
        let angle = Angle::new::<radian>(3.14);
        let neg_angle = Angle::new::<radian>(-1.0);
        let zero_angle = Angle::new::<radian>(0.0);

        assert!(angle.normal());
        assert!(angle.finite());
        assert!(!angle.zero());
        assert!(angle.sign_positive());
        assert!(!neg_angle.sign_positive());
        assert!(neg_angle.sign_negative());

        assert!(!zero_angle.normal());
        assert!(zero_angle.finite());
        assert!(zero_angle.zero());
    }
}
