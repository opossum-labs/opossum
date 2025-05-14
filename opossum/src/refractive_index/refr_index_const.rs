//! Trivial constant refractive index model
//!
//! This model simply returns a wavelength independant constant value.
use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

use super::{RefractiveIndex, RefractiveIndexType};
use crate::error::OpmResult;
use crate::error::OpossumError;

#[must_use]
#[allow(clippy::missing_panics_doc)]
/// Create a refractive index model representing vacuum.
///
/// This returns a constant (wavelength independant) refractive index of 1.0.
pub fn refr_index_vaccuum() -> RefractiveIndexType {
    RefractiveIndexType::Const(RefrIndexConst::new(1.0).unwrap())
}
/// Constant refractive index model
#[derive(Clone, Serialize, Deserialize, Debug, ToSchema)]
pub struct RefrIndexConst {
    refractive_index: f64,
}
impl RefrIndexConst {
    /// Create a new constant refrective index model.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given refractive index is < 1.0 or not finite.
    pub fn new(refractive_index: f64) -> OpmResult<Self> {
        if refractive_index < 1.0 || !refractive_index.is_finite() {
            return Err(OpossumError::Other(
                "refractive index must be >=1.0 and finite.".into(),
            ));
        }
        Ok(Self { refractive_index })
    }
}

impl RefractiveIndex for RefrIndexConst {
    fn get_refractive_index(&self, _wavelength: uom::si::f64::Length) -> OpmResult<f64> {
        Ok(self.refractive_index)
    }
    fn to_enum(&self) -> super::RefractiveIndexType {
        RefractiveIndexType::Const(self.clone())
    }
}
impl From<RefrIndexConst> for RefractiveIndexType {
    fn from(i: RefrIndexConst) -> Self {
        Self::Const(i)
    }
}
#[cfg(test)]
mod test {
    use num::Zero;
    use uom::si::f64::Length;

    use super::*;
    #[test]
    fn new() {
        assert!(RefrIndexConst::new(0.99).is_err());
        assert!(RefrIndexConst::new(f64::NAN).is_err());
        assert!(RefrIndexConst::new(f64::INFINITY).is_err());
    }
    #[test]
    fn get_refractive_index() {
        let i = RefrIndexConst::new(1.5).unwrap();
        assert_eq!(i.get_refractive_index(Length::zero()).unwrap(), 1.5);
    }
    #[test]
    fn get_enum() {
        let i = RefrIndexConst::new(1.5).unwrap();
        assert!(matches!(i.to_enum(), RefractiveIndexType::Const(_)));
    }
}
