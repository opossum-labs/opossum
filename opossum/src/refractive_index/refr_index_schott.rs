//! Schott model
use std::ops::Range;

use serde::Deserialize;
use serde::Serialize;
use uom::si::f64::Length;
use uom::si::length::micrometer;

use crate::error::OpmResult;
use crate::error::OpossumError;
use crate::nanometer;

use super::{RefractiveIndex, RefractiveIndexType};

/// Refractive index model following the Schott equation.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct RefrIndexSchott {
    a0: f64,
    a1: f64,
    a2: f64,
    a3: f64,
    a4: f64,
    a5: f64,
    wvl_range: Range<Length>,
}

impl Default for RefrIndexSchott {
    //H-ZF52
    fn default() -> Self {
        Self {
            a0: 3.267_600_58E+000,
            a1: -2.053_845_66E-002,
            a2: 3.515_076_72E-002,
            a3: 7.701_513_48E-003,
            a4: -9.081_398_17E-004,
            a5: 7.526_495_55E-005,
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
            a0,
            a1,
            a2,
            a3,
            a4,
            a5,
            wvl_range: wavelength_range,
        })
    }

    /// Returns the coefficient `a0` of the Schott equation.
    #[must_use]
    pub const fn a0(&self) -> f64 {
        self.a0
    }

    /// Sets the coefficient `a0` of the Schott equation.
    pub const fn set_a0(&mut self, value: f64) {
        self.a0 = value;
    }

    /// Returns the coefficient `a1` of the Schott equation.
    #[must_use]
    pub const fn a1(&self) -> f64 {
        self.a1
    }

    /// Sets the coefficient `a1` of the Schott equation.
    pub const fn set_a1(&mut self, value: f64) {
        self.a1 = value;
    }

    /// Returns the coefficient `a2` of the Schott equation.
    #[must_use]
    pub const fn a2(&self) -> f64 {
        self.a2
    }

    /// Sets the coefficient `a2` of the Schott equation.
    pub const fn set_a2(&mut self, value: f64) {
        self.a2 = value;
    }

    /// Returns the coefficient `a3` of the Schott equation.
    #[must_use]
    pub const fn a3(&self) -> f64 {
        self.a3
    }

    /// Sets the coefficient `a3` of the Schott equation.
    pub const fn set_a3(&mut self, value: f64) {
        self.a3 = value;
    }

    /// Returns the coefficient `a4` of the Schott equation.
    #[must_use]
    pub const fn a4(&self) -> f64 {
        self.a4
    }

    /// Sets the coefficient `a4` of the Schott equation.
    pub const fn set_a4(&mut self, value: f64) {
        self.a4 = value;
    }

    /// Returns the coefficient `a5` of the Schott equation.
    #[must_use]
    pub const fn a5(&self) -> f64 {
        self.a5
    }

    /// Sets the coefficient `a5` of the Schott equation.
    pub const fn set_a5(&mut self, value: f64) {
        self.a5 = value;
    }

    /// Returns the wavelength range for which the Schott equation is valid.
    #[must_use]
    pub fn wavelength_range(&self) -> &Range<Length> {
        &self.wvl_range
    }

    /// Sets the full wavelength range for which the Schott equation is valid.
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
impl RefractiveIndex for RefrIndexSchott {
    fn get_refractive_index(&self, wavelength: Length) -> OpmResult<f64> {
        if !self.wvl_range.contains(&wavelength) {
            return Err(OpossumError::Other("wavelength outside valid range".into()));
        }
        let lambda = wavelength.get::<micrometer>();
        Ok(f64::sqrt(
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
        ))
    }
    fn to_enum(&self) -> RefractiveIndexType {
        RefractiveIndexType::Schott(self.clone())
    }
}
impl From<RefrIndexSchott> for RefractiveIndexType {
    fn from(refr: RefrIndexSchott) -> Self {
        Self::Schott(refr)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::nanometer;
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
        assert_eq!(r.a0, 1.0);
        assert_eq!(r.a1, 2.0);
        assert_eq!(r.a2, 3.0);
        assert_eq!(r.a3, 4.0);
        assert_eq!(r.a4, 5.0);
        assert_eq!(r.a5, 6.0);
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
    #[test]
    fn get_enum() {
        let i = RefrIndexSchott::new(
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            nanometer!(1.0)..nanometer!(2.0),
        )
        .unwrap();
        assert!(matches!(i.to_enum(), RefractiveIndexType::Schott(_)));
    }
}
