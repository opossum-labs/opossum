//! Module for catalog internal transmittance datasets in `opossum_core`.

use crate::error::OpmResult;
use crate::generic_validators::{AllFinite, AllNotZero, AllPositive};
use crate::light::Spectrum;
use crate::{millimeter, validated, validated_type};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

type ValidatedLength = validated_type!(Length, AllFinite && AllPositive && AllNotZero);

impl Default for ValidatedLength {
    fn default() -> Self {
        validated!(millimeter!(10.0), AllFinite && AllPositive && AllNotZero).unwrap()
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
    /// This function returns an error if the given reference thickness is <= 0m or not finite.
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
    /// This function returns an error if the given reference thickness is <= 0m or not finite.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::micrometer;

    #[test]
    fn test_set_spectrum() -> OpmResult<()> {
        let mut act = AbsCatTrans::default();

        let mut new_spectrum = Spectrum::new(micrometer!(0.4)..micrometer!(0.8), micrometer!(0.1))?;
        new_spectrum.set_data(vec![(0.4, 0.95), (0.8, 0.85)])?;

        act.set_spectrum(new_spectrum.clone());
        assert_eq!(act.spectrum(), &new_spectrum);
        Ok(())
    }

    #[test]
    fn test_reference_thickness_validation() -> OpmResult<()> {
        let mut act = AbsCatTrans::default();
        let new_thickness = millimeter!(25.0);

        act.set_reference_thickness(new_thickness)?;
        assert_eq!(act.reference_thickness(), new_thickness);

        // Verify guardrails for non-positive or invalid thicknesses
        assert!(act.set_reference_thickness(millimeter!(-5.0)).is_err());
        assert!(act.set_reference_thickness(millimeter!(0.0)).is_err());
        assert!(act.set_reference_thickness(millimeter!(f64::NAN)).is_err());
        assert!(
            act.set_reference_thickness(millimeter!(f64::INFINITY))
                .is_err()
        );

        // Also test the constructor new() with invalid thickness
        let dummy_spectrum = Spectrum::default();
        assert!(AbsCatTrans::new(millimeter!(0.0), dummy_spectrum.clone()).is_err());
        assert!(AbsCatTrans::new(millimeter!(-1.0), dummy_spectrum).is_err());

        Ok(())
    }
}
