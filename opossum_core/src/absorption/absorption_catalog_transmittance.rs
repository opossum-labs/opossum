use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;
use crate::error::OpmResult;
use crate::{generic_validators::*, millimeter, validated, validated_vec_type};

use crate::{absorption::absorption_constant::ValidatedAbsConst, validated_type};
type ValidatedLength = validated_type!(Length, AllFinite && AllPositive);
impl Default for ValidatedLength {
    fn default() -> Self {
        validated!(millimeter!(10.0), AllFinite && AllPositive).unwrap()
    }
}
//( type ValidatedData= validated_vec_type!((ValidatedLength, ValidatedAbsConst));
#[derive(Default, Clone, Serialize, Deserialize, Debug, PartialEq, EnsureValidated)]
pub struct AbsCatTrans {
    /// The reference thickness for the given transmittance data.
    reference_thickness: ValidatedLength,
    /// Tabulated wavelength-transmittance pairs.
    #[validate(skip)]
    data: Vec<(ValidatedLength, ValidatedAbsConst)>,
}

impl AbsCatTrans {
  pub fn new(reference_thickness: Length, data: Vec<(Length,f64)>) -> OpmResult<Self> {
    let mut act=Self::default();
    act.reference_thickness.set(reference_thickness)?;
    //act.data.set
    Ok(act)
  }
}