use crate::error::OpmResult;
use crate::generic_validators::{AllFinite, AllPositive};
use crate::light::Spectrum;
use crate::{millimeter, validated, validated_type};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

type ValidatedLength = validated_type!(Length, AllFinite && AllPositive);

impl Default for ValidatedLength {
    fn default() -> Self {
        validated!(millimeter!(10.0), AllFinite && AllPositive).unwrap()
    }
}

/// Holds catalog internal transmittance data for a specific reference sample thickness.
#[derive(Default, Clone, Serialize, Deserialize, Debug, PartialEq, EnsureValidated)]
pub struct AbsCatTrans {
    /// The reference thickness for the given transmittance data.
    reference_thickness: ValidatedLength,
    /// Tabulated wavelength-transmittance spectrum.
    spectrum: Spectrum,
}

impl AbsCatTrans {
    /// Creates a new catalog transmittance dataset.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given reference thickness is <=0m or not finite
    pub fn new(reference_thickness: Length, spectrum: Spectrum) -> OpmResult<Self> {
        let mut act = Self::default();
        act.reference_thickness.set(reference_thickness)?;
        act.spectrum = spectrum;
        Ok(act)
    }

    /// Returns the reference sample thickness.
    #[must_use]
    pub const fn reference_thickness(&self) -> Length {
        *self.reference_thickness.get()
    }

    /// Updates the reference thickness with validation.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given reference thickness is <=0m or not finite.
    pub fn set_reference_thickness(&mut self, thickness: Length) -> OpmResult<()> {
        self.reference_thickness.set(thickness)
    }

    /// Returns a shared reference to the internal transmittance spectrum.
    #[must_use]
    pub const fn spectrum(&self) -> &Spectrum {
        &self.spectrum
    }

    /// Replaces the internal transmittance spectrum.
    pub fn set_spectrum(&mut self, spectrum: Spectrum) {
        self.spectrum = spectrum;
    }
}
