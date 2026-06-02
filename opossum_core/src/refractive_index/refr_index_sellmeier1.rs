//! Sellmeier 1 model
use std::ops::Range;

use serde::{Deserialize, Serialize};
use uom::si::f64::Length;
use uom::si::length::micrometer;

use crate::error::OpmResult;
use crate::error::OpossumError;
use crate::nanometer;

use super::RefractiveIndexType;
use super::bounded_model::{BoundedFormula, DispersionFormula};

/// Coefficients for the Sellmeier (1) dispersion formula.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Sellmeier1Coefficients {
    /// Coefficient K1
    pub k1: f64,
    /// Coefficient K2
    pub k2: f64,
    /// Coefficient K3
    pub k3: f64,
    /// Coefficient L1
    pub l1: f64,
    /// Coefficient L2
    pub l2: f64,
    /// Coefficient L3
    pub l3: f64,
}

impl DispersionFormula for Sellmeier1Coefficients {
    fn calculate(&self, wavelength: Length) -> f64 {
        let lambda = wavelength.get::<micrometer>();
        let l_sq = lambda * lambda;
        f64::sqrt(
            1.0 + self.k1 * l_sq / (l_sq - self.l1)
                + self.k2 * l_sq / (l_sq - self.l2)
                + self.k3 * l_sq / (l_sq - self.l3),
        )
    }
}

/// Sellmeier (1) model for calculation of a refractive index.
pub type RefrIndexSellmeier1 = BoundedFormula<Sellmeier1Coefficients>;

impl Default for RefrIndexSellmeier1 {
    //N-BK7
    fn default() -> Self {
        Self {
            coefficients: Sellmeier1Coefficients {
                k1: 1.039_612_120,
                k2: 0.231_792_344,
                k3: 1.010_469_450,
                l1: 0.006_000_698_67,
                l2: 0.020_017_914_4,
                l3: 103.560_653_0,
            },
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

        Self::from_coefficients(
            Sellmeier1Coefficients {
                k1,
                k2,
                k3,
                l1,
                l2,
                l3,
            },
            wavelength_range,
        )
    }

    /// Returns the coefficient `k1` of the Sellmeier equation.
    #[must_use]
    pub const fn k1(&self) -> f64 {
        self.coefficients.k1
    }

    /// Sets the coefficient `k1` of the Sellmeier equation.
    pub const fn set_k1(&mut self, value: f64) {
        self.coefficients.k1 = value;
    }

    /// Returns the coefficient `k2` of the Sellmeier equation.
    #[must_use]
    pub const fn k2(&self) -> f64 {
        self.coefficients.k2
    }

    /// Sets the coefficient `k2` of the Sellmeier equation.
    pub const fn set_k2(&mut self, value: f64) {
        self.coefficients.k2 = value;
    }

    /// Returns the coefficient `k3` of the Sellmeier equation.
    #[must_use]
    pub const fn k3(&self) -> f64 {
        self.coefficients.k3
    }

    /// Sets the coefficient `k3` of the Sellmeier equation.
    pub const fn set_k3(&mut self, value: f64) {
        self.coefficients.k3 = value;
    }

    /// Returns the coefficient `l1` (lambda squared denominator term).
    #[must_use]
    pub const fn l1(&self) -> f64 {
        self.coefficients.l1
    }

    /// Sets the coefficient `l1` (lambda squared denominator term).
    pub const fn set_l1(&mut self, value: f64) {
        self.coefficients.l1 = value;
    }

    /// Returns the coefficient `l2` (lambda squared denominator term).
    #[must_use]
    pub const fn l2(&self) -> f64 {
        self.coefficients.l2
    }

    /// Sets the coefficient `l2` (lambda squared denominator term).
    pub const fn set_l2(&mut self, value: f64) {
        self.coefficients.l2 = value;
    }

    /// Returns the coefficient `l3` (lambda squared denominator term).
    #[must_use]
    pub const fn l3(&self) -> f64 {
        self.coefficients.l3
    }

    /// Sets the coefficient `l3` (lambda squared denominator term).
    pub const fn set_l3(&mut self, value: f64) {
        self.coefficients.l3 = value;
    }
}

impl From<RefrIndexSellmeier1> for RefractiveIndexType {
    fn from(refr: RefrIndexSellmeier1) -> Self {
        Self::Sellmeier1(refr)
    }
}
#[cfg(test)]
mod test {
    use crate::{nanometer, refractive_index::RefractiveIndex};
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
    fn new() -> OpmResult<()> {
        let r = RefrIndexSellmeier1::new(
            1.0,
            2.0,
            3.0,
            4.0,
            5.0,
            6.0,
            nanometer!(500.0)..nanometer!(2000.0),
        )?;
        assert_eq!(r.k1(), 1.0);
        assert_eq!(r.k2(), 2.0);
        assert_eq!(r.k3(), 3.0);
        assert_eq!(r.l1(), 4.0);
        assert_eq!(r.l2(), 5.0);
        assert_eq!(r.l3(), 6.0);
        Ok(())
    }
    #[test]
    fn test_default_bk7() {
        let bk7 = RefrIndexSellmeier1::default();
        // Check if default is indeed N-BK7 (Schott)
        assert_eq!(bk7.k1(), 1.039_612_120);
        assert_eq!(bk7.l1(), 0.006_000_698_67);
        assert_eq!(bk7.wavelength_range().start, nanometer!(1000.0));
    }

    #[test]
    fn test_setters_and_getters() {
        let mut r = RefrIndexSellmeier1::default();

        // Test K-coefficients
        r.set_k1(1.5);
        r.set_k2(2.5);
        r.set_k3(3.5);
        assert_eq!(r.k1(), 1.5);
        assert_eq!(r.k2(), 2.5);
        assert_eq!(r.k3(), 3.5);

        // Test L-coefficients
        r.set_l1(0.01);
        r.set_l2(0.02);
        r.set_l3(0.03);
        assert_eq!(r.l1(), 0.01);
        assert_eq!(r.l2(), 0.02);
        assert_eq!(r.l3(), 0.03);

        // Test Wavelength Range setters
        let new_range = nanometer!(400.0)..nanometer!(800.0);
        r.set_wavelength_range(new_range.clone());
        assert_eq!(r.wavelength_range(), &new_range);
    }

    #[test]
    fn test_wavelength_range_logic() {
        let r = RefrIndexSellmeier1::default(); // 1000nm to 1100nm

        // Lower bound: inclusive
        assert!(r.get_refractive_index(nanometer!(1000.0)).is_ok());

        // Upper bound: exclusive (Rust Range behavior: start..end)
        assert!(r.get_refractive_index(nanometer!(1100.0)).is_err());
    }

    #[test]
    fn test_from_trait_integration() {
        let r = RefrIndexSellmeier1::default();
        let r_enum: RefractiveIndexType = r.clone().into();

        if let RefractiveIndexType::Sellmeier1(inner) = r_enum {
            assert_eq!(inner, r);
        } else {
            panic!("From trait conversion to RefractiveIndexType::Sellmeier1 failed");
        }
    }
    #[test]
    fn get_refractive_index() -> OpmResult<()> {
        let i = RefrIndexSellmeier1::new(
            6.14555251E-1,
            6.56775017E-1,
            1.02699346E+0,
            1.45987884E-2,
            2.87769588E-3,
            1.07653051E+2,
            nanometer!(500.0)..nanometer!(2000.0),
        )?;
        assert_relative_eq!(
            i.get_refractive_index(nanometer!(1054.0))?,
            1.5068,
            max_relative = 0.0001
        );
        assert!(i.get_refractive_index(nanometer!(499.0)).is_err());
        assert!(i.get_refractive_index(nanometer!(2001.0)).is_err());
        Ok(())
    }
}
