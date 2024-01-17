use uom::si::{f64::Length, length::nanometer};

use crate::{error::OpmResult, spectrum::Spectrum};

/// Helper function for generating a visible spectrum.
///
/// This function generates an empty spectrum in the visible range (350 - 750 nm) with a resolution
/// of 0.1 nm.
///
/// # Panics
/// This function might theoretically panic if the internal implementation of spectrum creation changes.
#[must_use]
pub fn create_visible_spec() -> Spectrum {
    Spectrum::new(
        Length::new::<nanometer>(380.0)..Length::new::<nanometer>(750.0),
        Length::new::<nanometer>(0.1),
    )
    .unwrap()
}
/// Helper function for generating a near infrared spectrum.
///
/// This function generates an empty spectrum in the near infrared range (800 - 2500 nm) with a resolution
/// of 0.1 nm.
///
/// # Panics
/// This function might theoretically panic if the internal implementation of spectrum creation changes.
#[must_use]
pub fn create_nir_spec() -> Spectrum {
    Spectrum::new(
        Length::new::<nanometer>(800.0)..Length::new::<nanometer>(2500.0),
        Length::new::<nanometer>(0.1),
    )
    .unwrap()
}
/// Helper function for generating a spectrum of a narrow-band Helium-Neon laser.
///
/// This function generates an spectrum in the visible range (350 - 750 nm) with a resolution
/// of 0.1 nm and a (spectrum resolution limited) laser line at 632.816 nm.
///
/// # Errors
///
/// This functions returns an [`OpossumError`] if the given energy is negative.
pub fn create_he_ne_spec(energy: f64) -> OpmResult<Spectrum> {
    let mut s = create_visible_spec();
    s.add_single_peak(Length::new::<nanometer>(632.816), energy)?;
    Ok(s)
}
/// Helper function for generating a spectrum of a narrow-band Nd:glass laser.
///
/// This function generates an spectrum in the near infrared range (800 - 2500 nm) with a resolution
/// of 0.1 nm and a (Lorentzian) laser line at 1054 nm with a width of 0.5 nm.
///
/// # Errors
///
/// This functions returns an [`OpossumError`] if the given energy is negative.
pub fn create_nd_glass_spec(energy: f64) -> OpmResult<Spectrum> {
    let mut s = create_nir_spec();
    s.add_lorentzian_peak(
        Length::new::<nanometer>(1054.0),
        Length::new::<nanometer>(0.5),
        energy,
    )?;
    Ok(s)
}
