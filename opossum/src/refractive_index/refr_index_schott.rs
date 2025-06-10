//! Schott model
use std::ops::Range;

use serde::Deserialize;
use serde::Serialize;
use uom::si::f64::Length;
use uom::si::length::micrometer;

use crate::error::OpmResult;
use crate::error::OpossumError;

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
