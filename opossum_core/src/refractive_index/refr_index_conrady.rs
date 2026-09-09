//! Conrady model
use std::ops::Range;

use num_traits::Pow;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::micrometer};

use crate::{
    error::{OpmResult, OpossumError},
    nanometer,
};

use super::{
    RefractiveIndexType,
    bounded_model::{BoundedFormula, DispersionFormula},
};

/// Coefficients for the Conrady dispersion formula.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ConradyCoefficients {
    /// Base refractive index n0
    pub n0: f64,
    /// Coefficient A
    pub a: f64,
    /// Coefficient B
    pub b: f64,
}

impl DispersionFormula for ConradyCoefficients {
    fn calculate(&self, wavelength: Length) -> f64 {
        let lambda = wavelength.get::<micrometer>();
        self.n0 + (self.a / lambda) + (self.b / lambda.pow(3.5))
    }
}

/// Refractive index model following the Conrady formula.
pub type RefrIndexConrady = BoundedFormula<ConradyCoefficients>;

impl Default for RefrIndexConrady {
    //SiO2
    fn default() -> Self {
        Self {
            coefficients: ConradyCoefficients {
                n0: 1.427,
                a: 11.1,
                b: 5.13e6,
            },
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

        Self::from_coefficients(ConradyCoefficients { n0, a, b }, wavelength_range)
    }

    /// Returns the constant term `n0` in the Conrady equation.
    #[must_use]
    pub const fn n0(&self) -> f64 {
        self.coefficients.n0
    }

    /// Sets the constant term `n0` in the Conrady equation.
    pub const fn set_n0(&mut self, value: f64) {
        self.coefficients.n0 = value;
    }

    /// Returns the coefficient `a` in the Conrady equation.
    #[must_use]
    pub const fn a(&self) -> f64 {
        self.coefficients.a
    }

    /// Sets the coefficient `a` in the Conrady equation.
    pub const fn set_a(&mut self, value: f64) {
        self.coefficients.a = value;
    }

    /// Returns the coefficient `b` in the Conrady equation.
    #[must_use]
    pub const fn b(&self) -> f64 {
        self.coefficients.b
    }

    /// Sets the coefficient `b` in the Conrady equation.
    pub const fn set_b(&mut self, value: f64) {
        self.coefficients.b = value;
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
    use crate::{nanometer, refractive_index::RefractiveIndex};
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
    fn new() -> OpmResult<()> {
        let r = RefrIndexConrady::new(1.0, 2.0, 3.0, nanometer!(500.0)..nanometer!(2000.0))?;
        assert_eq!(r.n0(), 1.0);
        assert_eq!(r.a(), 2.0);
        assert_eq!(r.b(), 3.0);
        Ok(())
    }
    #[test]
    fn test_default_sio2() {
        let sio2 = RefrIndexConrady::default();
        // Verify SiO2 default coefficients
        assert_eq!(sio2.n0(), 1.427);
        assert_eq!(sio2.a(), 11.1);
        assert_eq!(sio2.b(), 5.13e6);
        assert_eq!(sio2.wavelength_range().start, nanometer!(1000.0));
    }

    #[test]
    fn test_setters_and_getters() {
        let mut r = RefrIndexConrady::default();

        // Test coefficient setters
        r.set_n0(1.5);
        r.set_a(12.0);
        r.set_b(6.0e6);

        assert_eq!(r.n0(), 1.5);
        assert_eq!(r.a(), 12.0);
        assert_eq!(r.b(), 6.0e6);

        // Test wavelength range setters
        let new_range = nanometer!(400.0)..nanometer!(800.0);
        r.set_wavelength_range(new_range.clone());
        assert_eq!(r.wavelength_range(), &new_range);
    }

    #[test]
    fn test_range_boundary_behavior() {
        let r = RefrIndexConrady::default(); // Range: 1000nm to 1100nm

        // Start is inclusive
        assert!(r.get_refractive_index(nanometer!(1000.0)).is_ok());

        // End is exclusive
        assert!(r.get_refractive_index(nanometer!(1100.0)).is_err());
    }

    #[test]
    fn test_enum_conversion_consistency() {
        let r = RefrIndexConrady::default();
        let r_enum: RefractiveIndexType = r.clone().into();

        if let RefractiveIndexType::Conrady(inner) = r_enum {
            assert_eq!(inner, r);
        } else {
            panic!("Enum conversion for Conrady failed");
        }
    }
    #[test]
    fn get_refractive_index() -> OpmResult<()> {
        let i = RefrIndexConrady::new(1.0, 1.0, 1.0, nanometer!(500.0)..nanometer!(2000.0))?;
        assert_relative_eq!(
            i.get_refractive_index(nanometer!(1054.0))?,
            2.7806,
            max_relative = 0.0001
        );
        assert!(i.get_refractive_index(nanometer!(499.0)).is_err());
        assert!(i.get_refractive_index(nanometer!(2001.0)).is_err());
        Ok(())
    }
}
