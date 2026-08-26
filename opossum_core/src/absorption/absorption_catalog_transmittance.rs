use crate::error::OpmResult;
use crate::validated_type;
use crate::{
    generic_validators::*, millimeter, nanometer, validated, validated_vec, validated_vec_type,
};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;
type ValidatedLength = validated_type!(Length, AllFinite && AllPositive);
impl Default for ValidatedLength {
    fn default() -> Self {
        validated!(millimeter!(10.0), AllFinite && AllPositive).unwrap()
    }
}
type ValidatedData = validated_vec_type!(
        Vec<(Length, f64)>,
        XNormal && YFinite && AllPositive,
        AllNotEmpty);
impl Default for ValidatedData {
    fn default() -> Self {
        validated_vec!(vec![(nanometer!(1000.0), 1.0)], XNormal && YFinite && AllPositive,
        AllNotEmpty).unwrap()
    }
}
#[derive(Default, Clone, Serialize, Deserialize, Debug, PartialEq, EnsureValidated)]
pub struct AbsCatTrans {
    /// The reference thickness for the given transmittance data.
    reference_thickness: ValidatedLength,
    /// Tabulated wavelength-transmittance pairs.
    data: ValidatedData,
}

impl AbsCatTrans {
    pub fn new(reference_thickness: Length, data: Vec<(Length, f64)>) -> OpmResult<Self> {
        let mut act = Self::default();
        act.reference_thickness.set(reference_thickness)?;
        let converted_data = data
            .into_iter()
            .map(|(wvl, val)| {
                Ok((wvl, val))
            })
            .collect::<OpmResult<Vec<_>>>()?;

        // Validate container constraints (AllNotEmpty) and store
        act.data.set(converted_data)?;
        Ok(act)
    }
    pub fn reference_thickness(&self) -> Length {
        *self.reference_thickness.get()
    }
    #[must_use]
    pub fn data(&self) -> &Vec<(Length, f64)> {
        self.data.get()
    }
}
