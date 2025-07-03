//! Conrady model
use std::ops::Range;

use num::pow::Pow;
use serde::Deserialize;
use serde::Serialize;
use uom::si::f64::Length;
use uom::si::length::micrometer;

use crate::error::OpmResult;
use crate::error::OpossumError;
use crate::nanometer;

use super::{RefractiveIndex, RefractiveIndexType};

/// Refractive index model following the Conrady formula.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct RefrIndexConrady {
    n0: f64,
    a: f64,
    b: f64,
    wvl_range: Range<Length>,
}

impl Default for RefrIndexConrady {
    //SiO2
    fn default() -> Self {
        Self {
            n0: 1.427,
            a: 11.1,
            b: 5.13e6,
            wvl_range: nanometer!(1000.)..nanometer!(1100.),
        }
    }
}
impl RefrIndexConrady {
    /// Create a new refractive index model following the Conrady formula.
    ///
    /// This formula is extremely useful if only a few index / wavelength pairs are known and need to be fit to a
    /// smooth curve.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given coefficeints are not finite.
    pub fn new(n0: f64, a: f64, b: f64, wavelength_range: Range<Length>) -> OpmResult<Self> {
        if !n0.is_finite() || !a.is_finite() || !b.is_finite() {
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
            n0,
            a,
            b,
            wvl_range: wavelength_range,
        })
    }
    /// Returns the constant term `n0` in the Conrady equation.
    #[must_use]
    pub const fn n0(&self) -> f64 {
        self.n0
    }

    /// Sets the constant term `n0` in the Conrady equation.
    pub const fn set_n0(&mut self, value: f64) {
        self.n0 = value;
    }

    /// Returns the coefficient `a` in the Conrady equation.
    #[must_use]
    pub const fn a(&self) -> f64 {
        self.a
    }

    /// Sets the coefficient `a` in the Conrady equation.
    pub const fn set_a(&mut self, value: f64) {
        self.a = value;
    }

    /// Returns the coefficient `b` in the Conrady equation.
    #[must_use]
    pub const fn b(&self) -> f64 {
        self.b
    }

    /// Sets the coefficient `b` in the Conrady equation.
    pub const fn set_b(&mut self, value: f64) {
        self.b = value;
    }

    /// Returns the wavelength range (in meters) over which the Conrady equation is valid.
    #[must_use]
    pub fn wavelength_range(&self) -> &Range<Length> {
        &self.wvl_range
    }

    /// Sets the full wavelength range (in meters) for which the Conrady equation is valid.
    pub fn set_wavelength_range(&mut self, range: Range<Length>) {
        self.wvl_range = range;
    }

    /// Sets the start of the wavelength range (in meters).
    pub fn set_wavelength_range_start(&mut self, start: Length) {
        self.wvl_range.start = start;
    }

    /// Sets the end of the wavelength range (in meters).
    pub fn set_wavelength_range_end(&mut self, end: Length) {
        self.wvl_range.end = end;
    }
}
impl RefractiveIndex for RefrIndexConrady {
    fn get_refractive_index(&self, wavelength: Length) -> OpmResult<f64> {
        if !self.wvl_range.contains(&wavelength) {
            return Err(OpossumError::Other("wavelength outside valid range".into()));
        }
        let lambda = wavelength.get::<micrometer>();
        Ok(self.n0 + (self.a / lambda) + (self.b / lambda.pow(3.5)))
    }
    fn to_enum(&self) -> RefractiveIndexType {
        RefractiveIndexType::Conrady(self.clone())
    }
}
impl From<RefrIndexConrady> for RefractiveIndexType {
    fn from(refr: RefrIndexConrady) -> Self {
        Self::Conrady(refr)
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
        assert!(RefrIndexConrady::new(1.0, 1.0, f64::NAN, valid_wvl.clone()).is_err());
        assert!(RefrIndexConrady::new(1.0, 1.0, f64::INFINITY, valid_wvl.clone()).is_err());

        assert!(RefrIndexConrady::new(1.0, f64::NAN, 1.0, valid_wvl.clone()).is_err());
        assert!(RefrIndexConrady::new(1.0, f64::INFINITY, 1.0, valid_wvl.clone()).is_err());

        assert!(RefrIndexConrady::new(f64::NAN, 1.0, 1.0, valid_wvl.clone()).is_err());
        assert!(RefrIndexConrady::new(f64::INFINITY, 1.0, 1.0, valid_wvl.clone()).is_err());

        assert!(
            RefrIndexConrady::new(1.0, 1.0, 1.0, nanometer!(-1.0)..nanometer!(2000.0)).is_err()
        );
        assert!(
            RefrIndexConrady::new(1.0, 1.0, 1.0, nanometer!(f64::NAN)..nanometer!(2000.0)).is_err()
        );
        assert!(
            RefrIndexConrady::new(1.0, 1.0, 1.0, nanometer!(f64::INFINITY)..nanometer!(2000.0))
                .is_err()
        );

        assert!(
            RefrIndexConrady::new(1.0, 1.0, 1.0, nanometer!(1000.0)..nanometer!(-1.0)).is_err()
        );
        assert!(
            RefrIndexConrady::new(1.0, 1.0, 1.0, nanometer!(1000.0)..nanometer!(f64::NAN)).is_err()
        );
        assert!(
            RefrIndexConrady::new(1.0, 1.0, 1.0, nanometer!(1000.0)..nanometer!(f64::INFINITY))
                .is_err()
        );
    }
    #[test]
    fn new() {
        let r =
            RefrIndexConrady::new(1.0, 2.0, 3.0, nanometer!(500.0)..nanometer!(2000.0)).unwrap();
        assert_eq!(r.n0, 1.0);
        assert_eq!(r.a, 2.0);
        assert_eq!(r.b, 3.0);
    }
    #[test]
    fn get_refractive_index() {
        let i =
            RefrIndexConrady::new(1.0, 1.0, 1.0, nanometer!(500.0)..nanometer!(2000.0)).unwrap();
        assert_relative_eq!(
            i.get_refractive_index(nanometer!(1054.0)).unwrap(),
            2.7806,
            max_relative = 0.0001
        );
        assert!(i.get_refractive_index(nanometer!(499.0)).is_err());
        assert!(i.get_refractive_index(nanometer!(2001.0)).is_err());
    }
    #[test]
    fn get_enum() {
        let i = RefrIndexConrady::new(0.0, 0.0, 0.0, nanometer!(1.0)..nanometer!(2.0)).unwrap();
        assert!(matches!(i.to_enum(), RefractiveIndexType::Conrady(_)));
    }
}
