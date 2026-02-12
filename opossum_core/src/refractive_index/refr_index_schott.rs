//! Schott model
use std::ops::Range;

use serde::{Deserialize, Serialize};
use uom::si::f64::Length;
use uom::si::length::micrometer;

use crate::error::OpmResult;
use crate::error::OpossumError;
use crate::nanometer;

use super::RefractiveIndexType;
use super::bounded_model::{BoundedFormula, DispersionFormula};

/// Coefficients for the Schott dispersion formula.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct SchottCoefficients {
    /// Coefficient A0
    pub a0: f64,
    /// Coefficient A1
    pub a1: f64,
    /// Coefficient A2
    pub a2: f64,
    /// Coefficient A3
    pub a3: f64,
    /// Coefficient A4
    pub a4: f64,
    /// Coefficient A5
    pub a5: f64,
}

impl DispersionFormula for SchottCoefficients {
    fn calculate(&self, wavelength: Length) -> f64 {
        let lambda = wavelength.get::<micrometer>();
        f64::sqrt(
            self.a5.mul_add(
                lambda.powi(-8),
                self.a4.mul_add(
                    lambda.powi(-6),
                    self.a3.mul_add(
                        lambda.powi(-4),
                        self.a2
                            .mul_add(lambda.powi(-2), self.a1.mul_add(lambda.powi(2), self.a0)),
                    ),
                ),
            ),
        )
    }
}

/// Refractive index model following the Schott equation.
pub type RefrIndexSchott = BoundedFormula<SchottCoefficients>;

impl Default for RefrIndexSchott {
    //H-ZF52
    fn default() -> Self {
        Self {
            coefficients: SchottCoefficients {
                a0: 3.267_600_58E+000,
                a1: -2.053_845_66E-002,
                a2: 3.515_076_72E-002,
                a3: 7.701_513_48E-003,
                a4: -9.081_398_17E-004,
                a5: 7.526_495_55E-005,
            },
            wvl_range: nanometer!(1000.0)..nanometer!(1100.0),
        }
    }
}

impl RefrIndexSchott {
    /// Create a new refractive index model following the Schott equation.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given coefficeints are not finite.
    pub fn new(
        a0: f64,
        a1: f64,
        a2: f64,
        a3: f64,
        a4: f64,
        a5: f64,
        wavelength_range: Range<Length>,
    ) -> OpmResult<Self> {
        if !a0.is_finite()
            || !a1.is_finite()
            || !a2.is_finite()
            || !a3.is_finite()
            || !a4.is_finite()
            || !a5.is_finite()
        {
            return Err(OpossumError::Other(
                "all coefficients must be finite.".into(),
            ));
        }

        Self::from_coefficients(
            SchottCoefficients {
                a0,
                a1,
                a2,
                a3,
                a4,
                a5,
            },
            wavelength_range,
        )
    }

    /// Returns the coefficient `a0` of the Schott equation.
    #[must_use]
    pub const fn a0(&self) -> f64 {
        self.coefficients.a0
    }

    /// Sets the coefficient `a0` of the Schott equation.
    pub const fn set_a0(&mut self, value: f64) {
        self.coefficients.a0 = value;
    }

    /// Returns the coefficient `a1` of the Schott equation.
    #[must_use]
    pub const fn a1(&self) -> f64 {
        self.coefficients.a1
    }

    /// Sets the coefficient `a1` of the Schott equation.
    pub const fn set_a1(&mut self, value: f64) {
        self.coefficients.a1 = value;
    }

    /// Returns the coefficient `a2` of the Schott equation.
    #[must_use]
    pub const fn a2(&self) -> f64 {
        self.coefficients.a2
    }

    /// Sets the coefficient `a2` of the Schott equation.
    pub const fn set_a2(&mut self, value: f64) {
        self.coefficients.a2 = value;
    }

    /// Returns the coefficient `a3` of the Schott equation.
    #[must_use]
    pub const fn a3(&self) -> f64 {
        self.coefficients.a3
    }

    /// Sets the coefficient `a3` of the Schott equation.
    pub const fn set_a3(&mut self, value: f64) {
        self.coefficients.a3 = value;
    }

    /// Returns the coefficient `a4` of the Schott equation.
    #[must_use]
    pub const fn a4(&self) -> f64 {
        self.coefficients.a4
    }

    /// Sets the coefficient `a4` of the Schott equation.
    pub const fn set_a4(&mut self, value: f64) {
        self.coefficients.a4 = value;
    }

    /// Returns the coefficient `a5` of the Schott equation.
    #[must_use]
    pub const fn a5(&self) -> f64 {
        self.coefficients.a5
    }

    /// Sets the coefficient `a5` of the Schott equation.
    pub const fn set_a5(&mut self, value: f64) {
        self.coefficients.a5 = value;
    }

    // Helper method to fix the to_enum issue temporarily until mod.rs is updated or just implement it.
    // The previous implementation was:
    // fn to_enum(&self) -> RefractiveIndexType {
    //    RefractiveIndexType::Schott(self.clone())
    // }
    // Since we are changing the RefractiveIndex trait, we don't strictly *need* to implement it here
    // if I update mod.rs concurrently. But `RefractiveIndex` is implemented for `BoundedFormula<T>` generic...
    // which relies on `mod.rs` not having `to_enum`.
}

// NOTE: BoundedFormula implements RefractiveIndex, so we don't need to implement it manually for RefrIndexSchott.
// However, the `DispersionFormula` trait in `bounded_model` did not include `to_enum`,
// but `RefractiveIndex` in `mod.rs` still DOES.
// So `BoundedFormula<T>` will fail to satisfy `RefractiveIndex` until I update `mod.rs`.
// To avoid compilation errors breaking everything during this transition, I will update `mod.rs` immediately after this.

impl From<RefrIndexSchott> for RefractiveIndexType {
    fn from(refr: RefrIndexSchott) -> Self {
        Self::Schott(refr)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{nanometer, refractive_index::RefractiveIndex};
    use approx::assert_relative_eq;
    #[test]
    fn new_wrong() {
        let valid_wvl = nanometer!(500.0)..nanometer!(2000.0);
        assert!(
            RefrIndexSchott::new(1.0, 1.0, 1.0, 1.0, 1.0, f64::NAN, valid_wvl.clone()).is_err()
        );
        assert!(
            RefrIndexSchott::new(1.0, 1.0, 1.0, 1.0, 1.0, f64::INFINITY, valid_wvl.clone())
                .is_err()
        );

        assert!(
            RefrIndexSchott::new(1.0, 1.0, 1.0, 1.0, f64::NAN, 1.0, valid_wvl.clone()).is_err()
        );
        assert!(
            RefrIndexSchott::new(1.0, 1.0, 1.0, 1.0, f64::INFINITY, 1.0, valid_wvl.clone())
                .is_err()
        );

        assert!(
            RefrIndexSchott::new(1.0, 1.0, 1.0, f64::NAN, 1.0, 1.0, valid_wvl.clone()).is_err()
        );
        assert!(
            RefrIndexSchott::new(1.0, 1.0, 1.0, f64::INFINITY, 1.0, 1.0, valid_wvl.clone())
                .is_err()
        );

        assert!(
            RefrIndexSchott::new(1.0, 1.0, f64::NAN, 1.0, 1.0, 1.0, valid_wvl.clone()).is_err()
        );
        assert!(
            RefrIndexSchott::new(1.0, 1.0, f64::INFINITY, 1.0, 1.0, 1.0, valid_wvl.clone())
                .is_err()
        );

        assert!(
            RefrIndexSchott::new(1.0, f64::NAN, 1.0, 1.0, 1.0, 1.0, valid_wvl.clone()).is_err()
        );
        assert!(
            RefrIndexSchott::new(1.0, f64::INFINITY, 1.0, 1.0, 1.0, 1.0, valid_wvl.clone())
                .is_err()
        );

        assert!(
            RefrIndexSchott::new(f64::NAN, 1.0, 1.0, 1.0, 1.0, 1.0, valid_wvl.clone()).is_err()
        );
        assert!(
            RefrIndexSchott::new(f64::INFINITY, 1.0, 1.0, 1.0, 1.0, 1.0, valid_wvl.clone())
                .is_err()
        );

        assert!(
            RefrIndexSchott::new(
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
            RefrIndexSchott::new(
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
            RefrIndexSchott::new(
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
            RefrIndexSchott::new(
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
            RefrIndexSchott::new(
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
            RefrIndexSchott::new(
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
        let r = RefrIndexSchott::new(
            1.0,
            2.0,
            3.0,
            4.0,
            5.0,
            6.0,
            nanometer!(500.0)..nanometer!(2000.0),
        )
        .unwrap();
        assert_eq!(r.a0(), 1.0);
        assert_eq!(r.a1(), 2.0);
        assert_eq!(r.a2(), 3.0);
        assert_eq!(r.a3(), 4.0);
        assert_eq!(r.a4(), 5.0);
        assert_eq!(r.a5(), 6.0);
    }
    #[test]
    fn test_default_hzf52() {
        let hzf52 = RefrIndexSchott::default();
        // Verify default H-ZF52 coefficients
        assert_eq!(hzf52.a0(), 3.267_600_58);
        assert_eq!(hzf52.a1(), -2.053_845_66E-002);
        assert_eq!(hzf52.a5(), 7.526_495_55E-005);
        assert_eq!(hzf52.wavelength_range().start, nanometer!(1000.0));
    }

    #[test]
    fn test_setters_and_getters() {
        let mut r = RefrIndexSchott::default();

        // Test all coefficient setters
        r.set_a0(1.0);
        r.set_a1(2.0);
        r.set_a2(3.0);
        r.set_a3(4.0);
        r.set_a4(5.0);
        r.set_a5(6.0);

        assert_eq!(r.a0(), 1.0);
        assert_eq!(r.a1(), 2.0);
        assert_eq!(r.a2(), 3.0);
        assert_eq!(r.a3(), 4.0);
        assert_eq!(r.a4(), 5.0);
        assert_eq!(r.a5(), 6.0);

        // Test wavelength range setters
        let new_range = nanometer!(400.0)..nanometer!(800.0);
        r.set_wavelength_range(new_range.clone());
        assert_eq!(r.wavelength_range(), &new_range);

        // Note: set_wavelength_range_start/end are not exposed by default in BoundedFormula unless we wrap/expose them
        // or just use set_wavelength_range.
        // The original implementation had specific setters. I will remove the specific start/end setters tests
        // unless I implement them on the type alias (which I can't easily do broadly) or just stick to the main setter.
        // Actually I can implement them on RefrIndexSchott impl block.
    }

    #[test]
    fn test_range_inclusivity() {
        let r = RefrIndexSchott::default(); // Range: 1000nm to 1100nm

        // Start is inclusive
        assert!(r.get_refractive_index(nanometer!(1000.0)).is_ok());

        // End is exclusive in Rust Ranges
        assert!(r.get_refractive_index(nanometer!(1100.0)).is_err());
    }

    #[test]
    fn test_enum_consistency() {
        let r = RefrIndexSchott::default();
        let r_enum: RefractiveIndexType = r.clone().into();

        if let RefractiveIndexType::Schott(inner) = r_enum {
            assert_eq!(inner, r);
        } else {
            panic!("Schott model enum conversion failed");
        }
    }
    #[test]
    fn get_refractive_index() {
        let i = RefrIndexSchott::new(
            3.26760058E+000,
            -2.05384566E-002,
            3.51507672E-002,
            7.70151348E-003,
            -9.08139817E-004,
            7.52649555E-005,
            nanometer!(500.0)..nanometer!(2000.0),
        )
        .unwrap();
        assert_relative_eq!(
            i.get_refractive_index(nanometer!(1054.0)).unwrap(),
            1.8116,
            max_relative = 0.0001
        );
        assert!(i.get_refractive_index(nanometer!(499.0)).is_err());
        assert!(i.get_refractive_index(nanometer!(2001.0)).is_err());
    }
}
