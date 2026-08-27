mod band;
mod edge;
mod math;
mod spectral_builder;

pub use band::{BandFilter, BandFilterType};
pub use edge::{EdgeFilter, EdgeFilterType};
use opm_macros_lib::EnsureValidated;
pub use spectral_builder::SpectralFilterBuilder;
use uom::si::f64::Ratio;

use std::fmt::Display;

use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{AllFinite, StaticBounds, StaticInRange},
    light::Spectrum,
    percent,
    utils::default_from_name::DefaultFromName,
    validated, validated_type,
};
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};

#[derive(Copy, Clone, PartialEq, Debug, Eq)]
struct TransmissionBounds;

impl StaticBounds<Ratio> for TransmissionBounds {
    fn min() -> Ratio {
        percent!(0.0)
    }
    fn max() -> Ratio {
        percent!(100.0)
    }
    fn inclusive() -> bool {
        true
    }
}

type ValidatedTransmission = validated_type!(
    Ratio,
    AllFinite && StaticInRange::<Ratio, TransmissionBounds>
);
impl Default for ValidatedTransmission {
    fn default() -> Self {
        validated!(
            percent!(100.0),
            AllFinite && StaticInRange::<Ratio, TransmissionBounds>::default()
        )
        .unwrap()
    }
}

/// Config data for a constant filter type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, EnsureValidated)]
pub struct FilterConst {
    /// the constant transmission value of the filter. Must be between 0.0 and 1.0
    transmission: ValidatedTransmission,
}
impl FilterConst {
    /// Creates a new [`FilterConst`] with the given transmission value.
    ///
    /// # Errors
    ///
    /// Returns an error if the transmission value is not between 0.0 and 1.0 or if it is not a finite number.
    pub fn new(transmission: Ratio) -> OpmResult<Self> {
        let mut new_transmission = ValidatedTransmission::default();
        new_transmission.set(transmission)?;
        Ok(Self {
            transmission: new_transmission,
        })
    }
    /// Returns the transmission value of the filter.
    #[must_use]
    pub const fn transmission(&self) -> &Ratio {
        self.transmission.get()
    }
}
/// Config data builder for an [`IdealFilter`](crate::nodes::IdealFilter).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter)]
pub enum FilterTypeBuilder {
    /// a fixed (wavelength-independant) transmission value. Must be between 0.0 and 1.0
    Constant(FilterConst),
    /// filter based on given transmission spectrum.
    Spectrum(SpectralFilterBuilder),
}
impl Default for FilterTypeBuilder {
    fn default() -> Self {
        Self::Constant(FilterConst::default())
    }
}
impl Display for FilterTypeBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Constant(_) => write!(f, "Constant"),
            Self::Spectrum(_) => write!(f, "Spectral filter"),
        }
    }
}

impl FilterTypeBuilder {
    /// Constructs a [`FilterType`] object from the builder.
    ///
    /// # Returns
    /// - A [`FilterType`] instance corresponding to the variant
    /// # Errors
    /// Returns an error if the creation of a spectrum from a .csv fails.
    pub fn build(&self) -> OpmResult<FilterType> {
        match self {
            Self::Constant(c) => Ok(FilterType::Constant(c.clone())),
            Self::Spectrum(spectral_filter_builder) => {
                Ok(FilterType::Spectrum(spectral_filter_builder.build()?))
            }
        }
    }
}

impl DefaultFromName for FilterTypeBuilder {
    fn default_from_name(name: &str) -> Option<Self> {
        for ftb in Self::iter() {
            if name == format!("{ftb}") {
                match ftb {
                    Self::Constant(_) => {
                        return Some(Self::Constant(FilterConst::new(percent!(100.0)).unwrap()));
                    }
                    Self::Spectrum(_) => return Some(ftb),
                }
            }
        }
        None
    }
}

/// Config data for an [`IdealFilter`](crate::nodes::IdealFilter).
#[derive(Debug, Clone, PartialEq)]
pub enum FilterType {
    /// a fixed (wavelength-independant) transmission value. Must be between 0.0 and 1.0
    Constant(FilterConst),
    /// filter based on given transmission spectrum.
    Spectrum(Spectrum),
}
impl TryFrom<f64> for FilterTypeBuilder {
    type Error = OpossumError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Ok(Self::Constant(FilterConst::new(value.into())?))
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn new() {
        assert!(FilterConst::new(percent!(0.0)).is_ok());
        assert!(FilterConst::new(percent!(100.0)).is_ok());
        assert!(FilterConst::new(percent!(-0.1)).is_err());
        assert!(FilterConst::new(percent!(100.1)).is_err());
    }
}
