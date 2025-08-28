//! Sellmeier 1 model
use std::ops::Range;

use serde::Deserialize;
use serde::Serialize;
use uom::si::f64::Length;
use uom::si::length::micrometer;

use crate::error::OpmResult;
use crate::error::OpossumError;
use crate::nanometer;

use super::{RefractiveIndex, RefractiveIndexType};

/// Sellmeier (1) model for calculation of a refractive index.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct RefrIndexSellmeier1 {
    k1: f64,
    k2: f64,
    k3: f64,
    l1: f64,
    l2: f64,
    l3: f64,
    wvl_range: Range<Length>,
}

impl Default for RefrIndexSellmeier1 {
    //N-BK7
    fn default() -> Self {
        Self {
            k1: 1.039_612_120,
            k2: 0.231_792_344,
            k3: 1.010_469_450,
            l1: 0.006_000_698_67,
            l2: 0.020_017_914_4,
            l3: 103.560_653_0,
            wvl_range: nanometer!(1000.)..nanometer!(1100.),
        }
    }
}

impl RefrIndexSellmeier1 {
    /// Create a new refractive index model following the Sellmeier (1) equation.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given coefficients are not finite.
    pub fn new(
        k1: f64,
        k2: f64,
        k3: f64,
        l1: f64,
        l2: f64,
        l3: f64,
        wavelength_range: Range<Length>,
    ) -> OpmResult<Self> {
        if !k1.is_finite() || !k2.is_finite() || !k3.is_finite() {
            return Err(OpossumError::Other(
                "all k coefficients must be finite".into(),
            ));
        }
        if l1.is_sign_negative()
            || !l1.is_finite()
            || l2.is_sign_negative()
            || !l2.is_finite()
            || l3.is_sign_negative()
            || !l3.is_finite()
        {
            return Err(OpossumError::Other(
                "all l coefficients must be positive and finite.".into(),
            ));
        }
        if wavelength_range.start.is_sign_negative() || !wavelength_range.start.is_finite() {
            return Err(OpossumError::Other(
                "lower wavelength limit is invalid.".into(),
            ));
        }
        if wavelength_range.end.is_sign_negative() || !wavelength_range.end.is_finite() {
            return Err(OpossumError::Other(
                "upper wavelength limit is invalid.".into(),
            ));
        }
        Ok(Self {
            k1,
            k2,
            k3,
            l1,
            l2,
            l3,
            wvl_range: wavelength_range,
        })
    }
    /// Returns the coefficient `k1` of the Sellmeier equation.
    #[must_use]
    pub const fn k1(&self) -> f64 {
        self.k1
    }

    /// Sets the coefficient `k1` of the Sellmeier equation.
    pub const fn set_k1(&mut self, value: f64) {
        self.k1 = value;
    }

    /// Returns the coefficient `k2` of the Sellmeier equation.
    #[must_use]
    pub const fn k2(&self) -> f64 {
        self.k2
    }

    /// Sets the coefficient `k2` of the Sellmeier equation.
    pub const fn set_k2(&mut self, value: f64) {
        self.k2 = value;
    }

    /// Returns the coefficient `k3` of the Sellmeier equation.
    #[must_use]
    pub const fn k3(&self) -> f64 {
        self.k3
    }

    /// Sets the coefficient `k3` of the Sellmeier equation.
    pub const fn set_k3(&mut self, value: f64) {
        self.k3 = value;
    }

    /// Returns the coefficient `l1` (lambda squared denominator term).
    #[must_use]
    pub const fn l1(&self) -> f64 {
        self.l1
    }

    /// Sets the coefficient `l1` (lambda squared denominator term).
    pub const fn set_l1(&mut self, value: f64) {
        self.l1 = value;
    }

    /// Returns the coefficient `l2` (lambda squared denominator term).
    #[must_use]
    pub const fn l2(&self) -> f64 {
        self.l2
    }

    /// Sets the coefficient `l2` (lambda squared denominator term).
    pub const fn set_l2(&mut self, value: f64) {
        self.l2 = value;
    }

    /// Returns the coefficient `l3` (lambda squared denominator term).
    #[must_use]
    pub const fn l3(&self) -> f64 {
        self.l3
    }

    /// Sets the coefficient `l3` (lambda squared denominator term).
    pub const fn set_l3(&mut self, value: f64) {
        self.l3 = value;
    }

    /// Returns the wavelength range (in meters) over which the Sellmeier equation is valid.
    #[must_use]
    pub fn wavelength_range(&self) -> &Range<Length> {
        &self.wvl_range
    }

    /// Sets the wavelength range (in meters) for which the Sellmeier equation is valid.
    pub fn set_wavelength_range(&mut self, range: Range<Length>) {
        self.wvl_range = range;
    }

    /// Sets the start value of the wavelength range.
    ///
    /// # Arguments
    ///
    /// * `start` - The new start value of the wavelength range (in meters).
    pub fn set_wavelength_range_start(&mut self, start: Length) {
        self.wvl_range.start = start;
    }

    /// Sets the end value of the wavelength range.
    ///
    /// # Arguments
    ///
    /// * `end` - The new end value of the wavelength range (in meters).
    pub fn set_wavelength_range_end(&mut self, end: Length) {
        self.wvl_range.end = end;
    }
}
impl RefractiveIndex for RefrIndexSellmeier1 {
    fn get_refractive_index(&self, wavelength: uom::si::f64::Length) -> OpmResult<f64> {
        if !self.wvl_range.contains(&wavelength) {
            return Err(OpossumError::Other("wavelength outside valid range".into()));
        }
        let lambda = wavelength.get::<micrometer>();
        let l_sq = lambda * lambda;
        Ok(f64::sqrt(
            1.0 + self.k1 * l_sq / (l_sq - self.l1)
                + self.k2 * l_sq / (l_sq - self.l2)
                + self.k3 * l_sq / (l_sq - self.l3),
        ))
    }
    fn to_enum(&self) -> RefractiveIndexType {
        RefractiveIndexType::Sellmeier1(self.clone())
    }
}
impl From<RefrIndexSellmeier1> for RefractiveIndexType {
    fn from(refr: RefrIndexSellmeier1) -> Self {
        Self::Sellmeier1(refr)
    }
}
#[cfg(test)]
mod test {
    use crate::nanometer;
    use approx::assert_relative_eq;

    use super::*;
    #[test]
    fn new_wrong() {
        let valid_wvl = nanometer!(500.0)..nanometer!(2000.0);
        assert!(
            RefrIndexSellmeier1::new(1.0, 1.0, 1.0, 1.0, 1.0, -1.0, valid_wvl.clone()).is_err()
        );
        assert!(
            RefrIndexSellmeier1::new(1.0, 1.0, 1.0, 1.0, 1.0, f64::NAN, valid_wvl.clone()).is_err()
        );
        assert!(
            RefrIndexSellmeier1::new(1.0, 1.0, 1.0, 1.0, 1.0, f64::INFINITY, valid_wvl.clone())
                .is_err()
        );

        assert!(
            RefrIndexSellmeier1::new(1.0, 1.0, 1.0, 1.0, -1.0, 1.0, valid_wvl.clone()).is_err()
        );
        assert!(
            RefrIndexSellmeier1::new(1.0, 1.0, 1.0, 1.0, f64::NAN, 1.0, valid_wvl.clone()).is_err()
        );
        assert!(
            RefrIndexSellmeier1::new(1.0, 1.0, 1.0, 1.0, f64::INFINITY, 1.0, valid_wvl.clone())
                .is_err()
        );

        assert!(
            RefrIndexSellmeier1::new(1.0, 1.0, 1.0, -1.0, 1.0, 1.0, valid_wvl.clone()).is_err()
        );
        assert!(
            RefrIndexSellmeier1::new(1.0, 1.0, 1.0, f64::NAN, 1.0, 1.0, valid_wvl.clone()).is_err()
        );
        assert!(
            RefrIndexSellmeier1::new(1.0, 1.0, 1.0, f64::INFINITY, 1.0, 1.0, valid_wvl.clone())
                .is_err()
        );
        assert!(
            RefrIndexSellmeier1::new(
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
                nanometer!(-1.0)..nanometer!(2000.0)
            )
            .is_err()
        );
        assert!(
            RefrIndexSellmeier1::new(
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
                nanometer!(f64::NAN)..nanometer!(2000.0)
            )
            .is_err()
        );
        assert!(
            RefrIndexSellmeier1::new(
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
                nanometer!(f64::INFINITY)..nanometer!(2000.0)
            )
            .is_err()
        );

        assert!(
            RefrIndexSellmeier1::new(
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
                nanometer!(1000.0)..nanometer!(-1.0)
            )
            .is_err()
        );
        assert!(
            RefrIndexSellmeier1::new(
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
                nanometer!(1000.0)..nanometer!(f64::NAN)
            )
            .is_err()
        );
        assert!(
            RefrIndexSellmeier1::new(
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
                nanometer!(1000.0)..nanometer!(f64::INFINITY)
            )
            .is_err()
        );
    }
    #[test]
    fn new() {
        let r = RefrIndexSellmeier1::new(
            1.0,
            2.0,
            3.0,
            4.0,
            5.0,
            6.0,
            nanometer!(500.0)..nanometer!(2000.0),
        )
        .unwrap();
        assert_eq!(r.k1, 1.0);
        assert_eq!(r.k2, 2.0);
        assert_eq!(r.k3, 3.0);
        assert_eq!(r.l1, 4.0);
        assert_eq!(r.l2, 5.0);
        assert_eq!(r.l3, 6.0);
    }
    #[test]
    fn get_refractive_index() {
        let i = RefrIndexSellmeier1::new(
            6.14555251E-1,
            6.56775017E-1,
            1.02699346E+0,
            1.45987884E-2,
            2.87769588E-3,
            1.07653051E+2,
            nanometer!(500.0)..nanometer!(2000.0),
        )
        .unwrap();
        assert_relative_eq!(
            i.get_refractive_index(nanometer!(1054.0)).unwrap(),
            1.5068,
            max_relative = 0.0001
        );
        assert!(i.get_refractive_index(nanometer!(499.0)).is_err());
        assert!(i.get_refractive_index(nanometer!(2001.0)).is_err());
    }
    #[test]
    fn get_enum() {
        let r = RefrIndexSellmeier1::new(
            1.0,
            2.0,
            3.0,
            4.0,
            5.0,
            6.0,
            nanometer!(500.0)..nanometer!(2000.0),
        )
        .unwrap();
        assert!(matches!(r.to_enum(), RefractiveIndexType::Sellmeier1(_)));
    }
}
