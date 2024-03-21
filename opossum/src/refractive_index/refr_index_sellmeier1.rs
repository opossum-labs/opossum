use serde::Deserialize;
use serde::Serialize;
use uom::si::length::micrometer;

use crate::error::OpmResult;
use crate::error::OpossumError;

use super::{RefractiveIndex, RefractiveIndexType};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RefrIndexSellmeier1 {
    k1: f64,
    k2: f64,
    k3: f64,
    l1: f64,
    l2: f64,
    l3: f64,
}
impl RefrIndexSellmeier1 {
    /// Create a new refractive index model following the Sellmeier equation.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given coefficients are not finite.
    pub fn new(k1: f64, k2: f64, k3: f64, l1: f64, l2: f64, l3: f64) -> OpmResult<Self> {
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
        Ok(Self {
            k1,
            k2,
            k3,
            l1,
            l2,
            l3,
        })
    }
}
impl RefractiveIndex for RefrIndexSellmeier1 {
    fn get_refractive_index(&self, wavelength: uom::si::f64::Length) -> f64 {
        let lambda = wavelength.get::<micrometer>();
        let l_sq = lambda * lambda;
        f64::sqrt(
            1.0 + self.k1 * l_sq / (l_sq - self.l1)
                + self.k2 * l_sq / (l_sq - self.l2)
                + self.k3 * l_sq / (l_sq - self.l3),
        )
    }
    fn to_enum(&self) -> RefractiveIndexType {
        RefractiveIndexType::Sellmeier1(self.clone())
    }
}
#[cfg(test)]
mod test {
    use crate::nanometer;
    use approx::assert_relative_eq;

    use super::*;
    #[test]
    fn new_wrong() {
        assert!(RefrIndexSellmeier1::new(1.0, 1.0, 1.0, 1.0, 1.0, -1.0).is_err());
        assert!(RefrIndexSellmeier1::new(1.0, 1.0, 1.0, 1.0, 1.0, f64::NAN).is_err());
        assert!(RefrIndexSellmeier1::new(1.0, 1.0, 1.0, 1.0, 1.0, f64::INFINITY).is_err());

        assert!(RefrIndexSellmeier1::new(1.0, 1.0, 1.0, 1.0, -1.0, 1.0).is_err());
        assert!(RefrIndexSellmeier1::new(1.0, 1.0, 1.0, 1.0, f64::NAN, 1.0).is_err());
        assert!(RefrIndexSellmeier1::new(1.0, 1.0, 1.0, 1.0, f64::INFINITY, 1.0).is_err());

        assert!(RefrIndexSellmeier1::new(1.0, 1.0, 1.0, -1.0, 1.0, 1.0).is_err());
        assert!(RefrIndexSellmeier1::new(1.0, 1.0, 1.0, f64::NAN, 1.0, 1.0).is_err());
        assert!(RefrIndexSellmeier1::new(1.0, 1.0, 1.0, f64::INFINITY, 1.0, 1.0).is_err());
    }
    #[test]
    fn new() {
        let r = RefrIndexSellmeier1::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0).unwrap();
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
        )
        .unwrap();
        assert_relative_eq!(
            i.get_refractive_index(nanometer!(1054.0)),
            1.5068,
            max_relative = 0.0001
        );
    }
    #[test]
    fn get_enum() {
        let r = RefrIndexSellmeier1::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0).unwrap();
        assert!(matches!(r.to_enum(), RefractiveIndexType::Sellmeier1(_)));
    }
}
