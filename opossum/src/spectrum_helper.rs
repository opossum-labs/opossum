#![warn(missing_docs)]
//! helper functions for easier generation of spectra

use crate::{
    error::{OpmResult, OpossumError},
    nanometer,
    spectrum::Spectrum,
};
use log::warn;
use num::Zero;
use std::ops::Range;
use uom::si::{f64::Length, length::micrometer};

/// Helper function for generating a visible spectrum.
///
/// This function generates an empty spectrum in the visible range (350 - 750 nm) with a resolution
/// of 0.1 nm.
///
/// # Panics
/// This function might theoretically panic if the internal implementation of spectrum creation changes.
#[must_use]
pub fn create_visible_spec() -> Spectrum {
    Spectrum::new(nanometer!(380.0)..nanometer!(750.0), nanometer!(0.1)).unwrap()
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
    Spectrum::new(nanometer!(800.0)..nanometer!(2500.0), nanometer!(0.1)).unwrap()
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
    s.add_single_peak(nanometer!(632.816), energy)?;
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
    s.add_lorentzian_peak(nanometer!(1054.0), nanometer!(0.5), energy)?;
    Ok(s)
}

/// Filter type for the generation of filter spectra.
pub enum FilterType {
    /// Transmission spectrum of an ideal short-pass filter with a simple step profile.
    ///  
    /// Wavelengths below the given `cut_off` value are set to 1.0 (=full transmisson) while all other values are set to
    /// zero (= full absorptin). Note that the actual cut-off wavelength is truncated to the next wavelength bin in the given
    /// resolution. This filter type returns an error if the cut-off wavelength is outside the wavelength range.
    ShortPassStep {
        /// Cut-Off wavelength
        cut_off: Length,
    },
    /// Transmission spectrum of an ideal short-pass filter with a smooth (sinosoidal) step profile.
    ///
    /// The transmission for wavelengths <= `cut_off - width / 2` is
    /// 1.0 (=full transmisson) while wavelengths >= `cut_off + width / 2` is set to zero (= full absorption). In between the transmission
    /// follows a sinusoidal curve with value of 0.5 exactly at the cutoff wavelength. This filter type returns an error if the given
    /// width is <= 0.0 nm.
    ShortPassSmooth {
        /// Cut-Off wavelength
        cut_off: Length,
        /// Width of the sinusoidal transition part
        width: Length,
    },
    /// Transmission spectrum of an ideal long-pass filter with a simple step profile.
    ///
    /// For wavelengths above the given `cut-off` wavelength, values are set to 1.0 (=full transmisson) while all other values are set to
    /// zero (= full absorptin). Note that the actual cut-off wavelength is truncated to the next wavelength bin in the given
    /// resolution. This filter type returns an error if the cut-off wavelength is outside the wavelength range.
    LongPassStep {
        /// Cut-Off wavelength
        cut_off: Length,
    },
    /// Transmission spectrum of an ideal long-pass filter with a smooth (sinosoidal) step profile.
    ///
    /// The transmission for wavelengths <= `cutoff - width / 2` is 0.0 (=full absoprtion) while wavelength >= `cutoff + width / 2` is
    /// set to 1.0 (= full transmission). In between, the transmission follows a sinusoidal curve with a value of 0.5 exactly at the
    /// cutoff wavelength. This filter type returns an error if the given width is <= 0.0 nm. This filter type returns an error if the given
    /// width is <= 0.0 nm.
    LongPassSmooth {
        /// Cut-Off wavelength
        cut_off: Length,
        /// Width of the sinusoidal transition part
        width: Length,
    },
}
/// Generate a filter spectrum spectrum of a given filter type
///
/// This helper generates a transmission spectrum with the given range and resolution with a filter charcteristic by the given [`FilterType`].
///
/// # Warnings
///
/// This function emits a warning log if the given cut-off wavelength is outside the spectrum range.
///  
/// # Errors
///
/// This function will return an error if
///   - the given rage and / or resolution are invalid.
///   - the parameters for the specfic given [`FilterType`] are wrong.
pub fn generate_filter_spectrum(
    range: Range<Length>,
    resolution: Length,
    filter_type: &FilterType,
) -> OpmResult<Spectrum> {
    let mut s = Spectrum::new(range.clone(), resolution)?;
    match filter_type {
        FilterType::ShortPassStep { cut_off } => {
            if !range.contains(cut_off) {
                warn!("cut-off wavelength must be inside the spectrum range");
            }
            let mut cut_off_in_um = cut_off.get::<micrometer>();
            s.map_mut(|(lambda, _)| {
                if lambda < &mut cut_off_in_um {
                    (*lambda, 1.0)
                } else {
                    (*lambda, 0.0)
                }
            });
        }
        FilterType::ShortPassSmooth { cut_off, width } => {
            if width.is_zero() || !width.is_normal() || width.is_sign_negative() {
                return Err(OpossumError::Spectrum(
                    "width must be positive and finite".into(),
                ));
            }
            let cut_off_in_um = cut_off.get::<micrometer>();
            let width_in_um = width.get::<micrometer>();
            s.map_mut(|(lambda, _)| {
                if lambda <= &mut (cut_off_in_um - width_in_um / 2.0) {
                    (*lambda, 1.0)
                } else if lambda >= &mut (cut_off_in_um + width_in_um) {
                    (*lambda, 0.0)
                } else {
                    let angle = std::f64::consts::PI / width_in_um * (*lambda - cut_off_in_um);
                    (*lambda, 0.5f64.mul_add(-angle.sin(), 0.5))
                }
            });
        }
        FilterType::LongPassStep { cut_off } => {
            if !range.contains(cut_off) {
                warn!("cut-off wavelength must be inside the spectrum range");
            }
            let mut cut_off_in_um = cut_off.get::<micrometer>();
            s.map_mut(|(lambda, _)| {
                if lambda > &mut cut_off_in_um {
                    (*lambda, 1.0)
                } else {
                    (*lambda, 0.0)
                }
            });
        }
        FilterType::LongPassSmooth { cut_off, width } => {
            if width.is_zero() || !width.is_normal() || width.is_sign_negative() {
                return Err(OpossumError::Spectrum(
                    "width must be positive and finite".into(),
                ));
            }
            let cut_off_in_um = cut_off.get::<micrometer>();
            let width_in_um = width.get::<micrometer>();
            s.map_mut(|(lambda, _)| {
                if lambda <= &mut (cut_off_in_um - width_in_um / 2.0) {
                    (*lambda, 0.0)
                } else if lambda >= &mut (cut_off_in_um + width_in_um) {
                    (*lambda, 1.0)
                } else {
                    let angle = std::f64::consts::PI / width_in_um * (*lambda - cut_off_in_um);
                    (*lambda, 0.5f64.mul_add(angle.sin(), 0.5))
                }
            });
        }
    }
    Ok(s)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{micrometer, utils::test_helper::test_helper::check_logs};
    use num::Zero;
    use testing_logger;

    #[test]
    fn test_short_pass_filter() {
        testing_logger::setup();
        assert!(generate_filter_spectrum(
            micrometer!(1.0)..micrometer!(5.0),
            micrometer!(1.0),
            &FilterType::ShortPassStep {
                cut_off: micrometer!(7.0)
            }
        )
        .is_ok());
        check_logs(
            log::Level::Warn,
            vec!["cut-off wavelength must be inside the spectrum range"],
        );
        let s = generate_filter_spectrum(
            micrometer!(1.0)..micrometer!(5.0),
            micrometer!(1.0),
            &FilterType::ShortPassStep {
                cut_off: micrometer!(3.0),
            },
        )
        .unwrap();
        assert_eq!(s.get_value(&micrometer!(1.0)).unwrap(), 1.0);
        assert_eq!(s.get_value(&micrometer!(2.0)).unwrap(), 1.0);
        assert_eq!(s.get_value(&micrometer!(3.0)).unwrap(), 0.0);
        assert_eq!(s.get_value(&micrometer!(4.0)).unwrap(), 0.0);
    }

    #[test]
    fn test_long_pass_filter() {
        testing_logger::setup();
        assert!(generate_filter_spectrum(
            micrometer!(1.0)..micrometer!(5.0),
            micrometer!(1.0),
            &FilterType::LongPassStep {
                cut_off: micrometer!(7.0)
            }
        )
        .is_ok());
        check_logs(
            log::Level::Warn,
            vec!["cut-off wavelength must be inside the spectrum range"],
        );
        let s = generate_filter_spectrum(
            micrometer!(1.0)..micrometer!(5.0),
            micrometer!(1.0),
            &FilterType::LongPassStep {
                cut_off: micrometer!(3.0),
            },
        )
        .unwrap();
        assert_eq!(s.get_value(&micrometer!(1.0)).unwrap(), 0.0);
        assert_eq!(s.get_value(&micrometer!(2.0)).unwrap(), 0.0);
        assert_eq!(s.get_value(&micrometer!(3.0)).unwrap(), 0.0);
        assert_eq!(s.get_value(&micrometer!(4.0)).unwrap(), 1.0);
    }
    #[test]
    fn test_short_pass_smooth_filter() {
        let range = micrometer!(1.0)..micrometer!(5.0);
        let resolution = micrometer!(0.5);
        assert!(generate_filter_spectrum(
            range.clone(),
            resolution,
            &FilterType::ShortPassSmooth {
                cut_off: micrometer!(3.0),
                width: Length::zero()
            }
        )
        .is_err());
        assert!(generate_filter_spectrum(
            range.clone(),
            resolution,
            &FilterType::ShortPassSmooth {
                cut_off: micrometer!(3.0),
                width: micrometer!(-1.0)
            }
        )
        .is_err());
        let s = generate_filter_spectrum(
            range,
            resolution,
            &FilterType::ShortPassSmooth {
                cut_off: micrometer!(3.0),
                width: micrometer!(1.0),
            },
        )
        .unwrap();
        assert_eq!(s.get_value(&micrometer!(1.0)).unwrap(), 1.0);
        assert_eq!(s.get_value(&micrometer!(2.0)).unwrap(), 1.0);
        assert_eq!(s.get_value(&micrometer!(2.5)).unwrap(), 1.0);
        assert_eq!(s.get_value(&micrometer!(3.0)).unwrap(), 0.5);
        assert_eq!(s.get_value(&micrometer!(3.5)).unwrap(), 0.0);
        assert_eq!(s.get_value(&micrometer!(4.0)).unwrap(), 0.0);
    }
    #[test]
    fn test_long_pass_smooth_filter() {
        let range = micrometer!(1.0)..micrometer!(5.0);
        let resolution = micrometer!(0.5);
        assert!(generate_filter_spectrum(
            range.clone(),
            resolution,
            &FilterType::LongPassSmooth {
                cut_off: micrometer!(3.0),
                width: Length::zero()
            }
        )
        .is_err());
        assert!(generate_filter_spectrum(
            range.clone(),
            resolution,
            &FilterType::LongPassSmooth {
                cut_off: micrometer!(3.0),
                width: micrometer!(-1.0)
            }
        )
        .is_err());
        let s = generate_filter_spectrum(
            range,
            resolution,
            &FilterType::LongPassSmooth {
                cut_off: micrometer!(3.0),
                width: micrometer!(1.0),
            },
        )
        .unwrap();
        assert_eq!(s.get_value(&micrometer!(1.0)).unwrap(), 0.0);
        assert_eq!(s.get_value(&micrometer!(2.0)).unwrap(), 0.0);
        assert_eq!(s.get_value(&micrometer!(2.5)).unwrap(), 0.0);
        assert_eq!(s.get_value(&micrometer!(3.0)).unwrap(), 0.5);
        assert_eq!(s.get_value(&micrometer!(3.5)).unwrap(), 1.0);
        assert_eq!(s.get_value(&micrometer!(4.0)).unwrap(), 1.0);
    }
}
