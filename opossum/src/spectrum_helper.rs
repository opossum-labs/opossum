#![warn(missing_docs)]
//! helper functions for easier generation of spectra

use crate::error::OpossumError;
use crate::{error::OpmResult, spectrum::Spectrum};
use std::ops::Range;
use uom::si::{
    f64::Length,
    length::{micrometer, nanometer},
};

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

pub enum FilterType {
    ShortPassStep(Length),
    ShortPassSmooth(Length, Length),
    LongPassStep(Length),
    LongPassSmooth(Length, Length),
}
pub fn generate_filter_spectrum(
    range: Range<Length>,
    resolution: Length,
    filter_type: FilterType,
) -> OpmResult<Spectrum> {
    let mut s = Spectrum::new(range, resolution)?;
    match filter_type {
        FilterType::ShortPassStep(cut_off) => {
            let mut cut_off_in_um = cut_off.get::<micrometer>();
            s.map_mut(|(lambda, _)| {
                if lambda < &mut cut_off_in_um {
                    (*lambda, 1.0)
                } else {
                    (*lambda, 0.0)
                }
            });
        }
        FilterType::ShortPassSmooth(cut_off, width) => todo!(),
        FilterType::LongPassStep(cut_off) => {
            let mut cut_off_in_um = cut_off.get::<micrometer>();
            s.map_mut(|(lambda, _)| {
                if lambda > &mut cut_off_in_um {
                    (*lambda, 1.0)
                } else {
                    (*lambda, 0.0)
                }
            });
        }
        FilterType::LongPassSmooth(cut_off, width) => todo!(),
    }
    Ok(s)
}
/// Generate a spectrum of an ideal short-pass filter.
///
/// This helper generates a transmission spectrum with the given range and resolution representing a short-pass filter.
/// Wavelengths below the given cut-off wavelength are set to 1.0 (=full transmisson) while all other values are set to
/// zero (= full absorptin). Note that the actual cut-off wavelength is truncated to the next wavelength bin in the given
/// resolution.
///  
/// # Errors
///
/// This function will return an error if
///   - the given rage and / or resolution are invalid.
///   - the cut-off wavelength is outside the spectrum range.
pub fn create_short_pass_filter(
    range: Range<Length>,
    resolution: Length,
    cut_off: Length,
) -> OpmResult<Spectrum> {
    if !range.contains(&cut_off) {
        return Err(OpossumError::Spectrum(
            "cut-off wavelength must be inside the spectrum range".into(),
        ));
    }
    let mut cut_off_in_um = cut_off.get::<micrometer>();
    let mut s = Spectrum::new(range, resolution)?;
    s.map_mut(|(lambda, _)| {
        if lambda < &mut cut_off_in_um {
            (*lambda, 1.0)
        } else {
            (*lambda, 0.0)
        }
    });
    Ok(s)
}

/// Generate a spectrum of an ideal long-pass filter.
///
/// This helper generates a transmission spectrum with the given range and resolution representing a long-pass filter.
/// Wavelengths above the given cut-off wavelength are set to 1.0 (=full transmisson) while all other values are set to
/// zero (= full absorptin). Note that the actual cut-off wavelength is truncated to the next wavelength bin in the given
/// resolution.
///  
/// # Errors
///
/// This function will return an error if
///   - the given rage and / or resolution are invalid.
///   - the cut-off wavelength is outside the spectrum range.
pub fn create_long_pass_filter(
    range: Range<Length>,
    resolution: Length,
    cut_off: Length,
) -> OpmResult<Spectrum> {
    if !range.contains(&cut_off) {
        return Err(OpossumError::Spectrum(
            "cut-off wavelength must be inside the spectrum range".into(),
        ));
    }
    let mut cut_off_in_um = cut_off.get::<micrometer>();
    let mut s = Spectrum::new(range, resolution)?;
    s.map_mut(|(lambda, _)| {
        if lambda > &mut cut_off_in_um {
            (*lambda, 1.0)
        } else {
            (*lambda, 0.0)
        }
    });
    Ok(s)
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_create_short_pass_filter() {
        assert!(create_short_pass_filter(
            Length::new::<micrometer>(1.0)..Length::new::<micrometer>(5.0),
            Length::new::<micrometer>(1.0),
            Length::new::<micrometer>(7.0)
        )
        .is_err());
        let s = create_short_pass_filter(
            Length::new::<micrometer>(1.0)..Length::new::<micrometer>(5.0),
            Length::new::<micrometer>(1.0),
            Length::new::<micrometer>(3.0),
        )
        .unwrap();
        assert_eq!(s.get_value(&Length::new::<micrometer>(1.0)).unwrap(), 1.0);
        assert_eq!(s.get_value(&Length::new::<micrometer>(2.0)).unwrap(), 1.0);
        assert_eq!(s.get_value(&Length::new::<micrometer>(3.0)).unwrap(), 0.0);
        assert_eq!(s.get_value(&Length::new::<micrometer>(4.0)).unwrap(), 0.0);
    }

    #[test]
    fn test_create_long_pass_filter() {
        assert!(create_long_pass_filter(
            Length::new::<micrometer>(1.0)..Length::new::<micrometer>(5.0),
            Length::new::<micrometer>(1.0),
            Length::new::<micrometer>(7.0)
        )
        .is_err());
        let s = create_long_pass_filter(
            Length::new::<micrometer>(1.0)..Length::new::<micrometer>(5.0),
            Length::new::<micrometer>(1.0),
            Length::new::<micrometer>(3.0),
        )
        .unwrap();
        assert_eq!(s.get_value(&Length::new::<micrometer>(1.0)).unwrap(), 0.0);
        assert_eq!(s.get_value(&Length::new::<micrometer>(2.0)).unwrap(), 0.0);
        assert_eq!(s.get_value(&Length::new::<micrometer>(3.0)).unwrap(), 0.0);
        assert_eq!(s.get_value(&Length::new::<micrometer>(4.0)).unwrap(), 1.0);
    }
}
