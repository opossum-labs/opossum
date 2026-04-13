use std::{fmt::Display, ops::Range, str::FromStr};

use log::warn;
use serde::{Deserialize, Serialize};
use strum::EnumIter;
use uom::si::{f64::Length, length::micrometer};

use crate::{
    error::{OpmResult, OpossumError},
    light::Spectrum,
    nanometer,
    nodes::ideal_filter::filter_types::math::interpolate_transition,
    utils::default_from_name::DefaultFromName,
};

/// Specifies the type of band filter.
///
/// - `BandPass`: Passes a specified wavelength band and attenuates others.
/// - `Notch`: Filters out a specified wavelength band while passing others.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter, Eq, Copy, Default)]
pub enum BandFilterType {
    /// Passes a specified wavelength band.
    #[default]
    BandPass,

    /// filters out a specified wavelength band.
    Notch,
}

impl Display for BandFilterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BandPass => write!(f, "Band pass"),
            Self::Notch => write!(f, "Notch"),
        }
    }
}

impl FromStr for BandFilterType {
    type Err = OpossumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::default_from_name(s).map_or_else(
            || {
                Err(OpossumError::Other(
                    "Invalid str identifier to create BandFilterType from string!".into(),
                ))
            },
            Ok,
        )
    }
}

impl DefaultFromName for BandFilterType {}

/// Represents a band filter with defined spectral characteristics.
///
/// A `BandFilter` describes either a band-pass or notch filter. It includes
/// parameters such as center wavelength, filter width, optional smooth transition width,
/// the operational wavelength range, and the spectral resolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BandFilter {
    /// The type of band filter (band-pass or notch).
    filter_type: BandFilterType,

    /// The central wavelength of the band.
    center_wavelength: Length,

    /// The full width of the band.
    width: Length,

    /// The minimum and maximum transmission values of the `BandFilter`
    transmission_range: Range<f64>,

    /// Optional smooth transition width at the band edges.
    ///
    /// If `Some`, the filter transitions gradually; if `None`, the transition is sharp.
    smooth_step_width: Option<Length>,

    /// The wavelength range over which the filter is defined.
    range: Range<Length>,

    /// The wavelength resolution associated with the filter's data.
    resolution: Length,
}

impl Default for BandFilter {
    fn default() -> Self {
        Self {
            filter_type: BandFilterType::BandPass,
            center_wavelength: nanometer!(1054.),
            width: nanometer!(10.),
            transmission_range: (0.)..1.,
            smooth_step_width: Some(nanometer!(2.)),
            range: nanometer!(1000.)..nanometer!(1100.),
            resolution: nanometer!(0.1),
        }
    }
}

impl BandFilter {
    /// Creates a new `BandFilter` instance.
    ///
    /// # Parameters
    /// - `band_filter_type`: The type of the band filter.
    /// - `center_wavelength`: The center wavelength of the filter. Must be positive and finite.
    /// - `width`: The width of the filter. Must be positive and finite.
    /// - `range`: The wavelength range. Start and end must be positive, finite, and `end` must be greater than `start`.
    /// - `resolution`: The resolution of the filter. Must be positive and finite.
    ///
    /// # Returns
    /// A new `BandFilter` instance wrapped in `Ok` if all parameters are valid.
    ///
    /// # Errors
    /// Returns an `OpossumError::Other` if any provided parameter is invalid.
    pub fn new(
        band_filter_type: BandFilterType,
        center_wavelength: Length,
        width: Length,
        transmission_range: Range<f64>,
        mut smooth_step_width: Option<Length>,
        range: Range<Length>,
        resolution: Length,
    ) -> OpmResult<Self> {
        if !center_wavelength.is_normal() || center_wavelength.is_sign_negative() {
            return Err(OpossumError::Other(
                "Center wavelength of Band-Filter must be positive and finite!".into(),
            ));
        }
        if !width.is_normal() || width.is_sign_negative() {
            return Err(OpossumError::Other(
                "Width of Band-Filter must be positive and finite!".into(),
            ));
        }
        if !resolution.is_normal() || resolution.is_sign_negative() {
            return Err(OpossumError::Other(
                "Resolution of Band-Filter must be positive and finite!".into(),
            ));
        }
        if !range.start.is_normal() || range.start.is_sign_negative() {
            return Err(OpossumError::Other(
                "Wavelength-range start of Band-Filter must be positive and finite!".into(),
            ));
        }
        if !range.end.is_normal() || range.end.is_sign_negative() || range.end <= range.start {
            return Err(OpossumError::Other("Wavelength-range end of Band-Filter must be positive, finite and larger than its start!".into()));
        }
        if transmission_range.start > 1. || transmission_range.end.is_sign_negative() {
            return Err(OpossumError::Other("Transmission minimum of Band-Filter must be positive, smaller than 1. and greater than 0!".into()));
        }
        if transmission_range.end > 1.
            || transmission_range.end.is_sign_negative()
            || transmission_range.end <= transmission_range.start
        {
            return Err(OpossumError::Other("Transmission maximum of Band-Filter must be positive, smaller than 1., greater than 0 and greater than the transmission minimum!".into()));
        }
        if !range.contains(&center_wavelength) {
            return Err(OpossumError::Other(
                "cut-off / cut-on wavelength must be inside the spectrum range".into(),
            ));
        }
        if let Some(smooth_width) = smooth_step_width.as_mut() {
            if !smooth_width.is_normal() || smooth_width.is_sign_negative() {
                return Err(OpossumError::Other(
                    "Step width must be positive and finite when provided!".into(),
                ));
            }
            if *smooth_width > width {
                warn!(
                    "Smoothing width is larger than actual filter width! Resetting to maximum smoothing width"
                );
                *smooth_width = width;
            }
        }

        Ok(Self {
            filter_type: band_filter_type,
            center_wavelength,
            width,
            transmission_range,
            smooth_step_width,
            range,
            resolution,
        })
    }

    // Returns the current band filter type.
    ///
    /// # Returns
    /// The `BandFilterType` of this filter.
    #[must_use]
    pub const fn band_filter_type(&self) -> &BandFilterType {
        &self.filter_type
    }

    /// Sets the band filter type.
    ///
    /// # Parameters
    /// - `band_filter_type`: The new filter type.
    pub const fn set_band_filter_type(&mut self, band_filter_type: BandFilterType) {
        self.filter_type = band_filter_type;
    }

    /// Returns the center wavelength.
    ///
    /// # Returns
    /// The center wavelength as `Length`.
    #[must_use]
    pub fn center_wavelength(&self) -> Length {
        self.center_wavelength
    }

    /// Sets the center wavelength.
    ///
    /// # Parameters
    /// - `center_wavelength`: The new center wavelength.
    ///
    /// # Errors
    /// Returns an error if the value is not positive and finite.
    pub fn set_center_wavelength(&mut self, center_wavelength: Length) -> OpmResult<()> {
        if !center_wavelength.is_normal() || center_wavelength.is_sign_negative() {
            return Err(OpossumError::Other(
                "Center wavelength of Band-Filter must be positive and finite!".into(),
            ));
        }
        self.center_wavelength = center_wavelength;
        Ok(())
    }

    /// Returns the filter width.
    ///
    /// # Returns
    /// The filter width as `Length`.
    #[must_use]
    pub fn width(&self) -> Length {
        self.width
    }

    /// Sets the filter width.
    ///
    /// # Parameters
    /// - `width`: The new filter width.
    ///
    /// # Errors
    /// Returns an error if the value is not positive and finite.
    pub fn set_width(&mut self, width: Length) -> OpmResult<()> {
        if !width.is_normal() || width.is_sign_negative() {
            return Err(OpossumError::Other(
                "Width of Band-Filter must be positive and finite!".into(),
            ));
        }
        self.width = width;
        Ok(())
    }

    /// Returns the optional step width.
    #[must_use]
    pub fn smooth_step_width(&self) -> Option<Length> {
        self.smooth_step_width
    }

    /// Sets the step width.
    ///
    /// # Parameters
    /// - `step_width`: The new step width or `None`.
    ///
    /// # Errors
    /// Returns an error if the provided value is not positive and finite.
    pub fn set_smooth_step_width(&mut self, mut step_width: Option<Length>) -> OpmResult<()> {
        if let Some(width) = &mut step_width
            && (!width.is_normal() || width.is_sign_negative())
        {
            return Err(OpossumError::Other(
                "Step width must be positive and finite when provided!".into(),
            ));
        }

        self.smooth_step_width = step_width;
        Ok(())
    }

    /// Returns the filter resolution.
    ///
    /// # Returns
    /// The filter resolution as `Length`.
    #[must_use]
    pub fn resolution(&self) -> Length {
        self.resolution
    }

    /// Sets the filter resolution.
    ///
    /// # Parameters
    /// - `resolution`: The new filter resolution.
    ///
    /// # Errors
    /// Returns an error if the value is not positive and finite.
    pub fn set_resolution(&mut self, resolution: Length) -> OpmResult<()> {
        if !resolution.is_normal() || resolution.is_sign_negative() {
            return Err(OpossumError::Other(
                "Resolution of Band-Filter must be positive and finite!".into(),
            ));
        }
        self.resolution = resolution;
        Ok(())
    }

    /// Returns the wavelength range.
    ///
    /// # Returns
    /// The wavelength range as `Range<Length>`.
    #[must_use]
    pub fn range(&self) -> Range<Length> {
        self.range.clone()
    }

    /// Sets the wavelength range.
    ///
    /// # Parameters
    /// - `range`: The new wavelength range.
    ///
    /// # Errors
    /// Returns an error if the range start or end is invalid.
    pub fn set_range(&mut self, range: Range<Length>) -> OpmResult<()> {
        if !range.start.is_normal() || range.start.is_sign_negative() {
            return Err(OpossumError::Other(
                "Wavelength-range start must be positive and finite!".into(),
            ));
        }
        if !range.end.is_normal() || range.end.is_sign_negative() || range.end <= range.start {
            return Err(OpossumError::Other(
                "Wavelength-range end must be positive, finite, and greater than start!".into(),
            ));
        }
        self.range = range;
        Ok(())
    }
    /// Sets the wavelength range start.
    ///
    /// # Parameters
    /// - `start`: The new start of the wavelength range.
    ///
    /// # Errors
    /// Returns an error if the start is invalid.
    pub fn set_range_start(&mut self, start: Length) -> OpmResult<()> {
        if !start.is_normal() || start.is_sign_negative() {
            return Err(OpossumError::Other(
                "Wavelength-range start must be positive and finite!".into(),
            ));
        }
        if self.range.end <= start {
            return Err(OpossumError::Other(
                "Wavelength-range start smaller than end!".into(),
            ));
        }

        self.range.start = start;
        Ok(())
    }

    /// Sets the wavelength range end.
    ///
    /// # Parameters
    /// - `end`: The new end of the wavelength range.
    ///
    /// # Errors
    /// Returns an error if the end is invalid.
    pub fn set_range_end(&mut self, end: Length) -> OpmResult<()> {
        if !end.is_normal() || end.is_sign_negative() {
            return Err(OpossumError::Other(
                "Wavelength-range end must be positive and finite!".into(),
            ));
        }
        if end <= self.range.start {
            return Err(OpossumError::Other(
                "Wavelength-range end must be greater than start!".into(),
            ));
        }

        self.range.end = end;
        Ok(())
    }

    /// Returns the transmission range.
    ///
    /// # Returns
    /// The transmission range as `Range<f64>`.
    #[must_use]
    pub fn transmission_range(&self) -> Range<f64> {
        self.transmission_range.clone()
    }

    /// Sets the transmission range.
    ///
    /// # Parameters
    /// - `transmission`: The new transmission range.
    ///
    /// # Errors
    /// Returns an error if the range start or end is invalid.
    pub fn set_transmission(&mut self, transmission_range: Range<f64>) -> OpmResult<()> {
        if transmission_range.start > 1. || transmission_range.end.is_sign_negative() {
            return Err(OpossumError::Other("Transmission minimum of Band-Filter must be positive, smaller than 1. and greater than 0!".into()));
        }
        if transmission_range.end > 1.
            || transmission_range.end.is_sign_negative()
            || transmission_range.end <= transmission_range.start
        {
            return Err(OpossumError::Other("Transmission maximum of Band-Filter must be positive, smaller than 1., greater than 0 and greater than the transmission minimum!".into()));
        }
        self.transmission_range = transmission_range;
        Ok(())
    }
    /// Sets the transmission range start.
    ///
    /// # Parameters
    /// - `start`: The new start of the transmission range.
    ///
    /// # Errors
    /// Returns an error if the start is invalid.
    pub fn set_transmission_range_start(&mut self, start: f64) -> OpmResult<()> {
        if start > 1. || start.is_sign_negative() {
            return Err(OpossumError::Other(
                "transmission-range start must be positive and finite!".into(),
            ));
        }
        if self.transmission_range.end <= start {
            return Err(OpossumError::Other(
                "transmission-range start must be smaller than its end!".into(),
            ));
        }

        self.transmission_range.start = start;
        Ok(())
    }

    /// Sets the wavetransmissionlength range end.
    ///
    /// # Parameters
    /// - `end`: The new end of the transmission range.
    ///
    /// # Errors
    /// Returns an error if the end is invalid.
    pub fn set_transmission_range_end(&mut self, end: f64) -> OpmResult<()> {
        if end > 1. || end.is_sign_negative() {
            return Err(OpossumError::Other(
                "transmission-range end must be positive and finite!".into(),
            ));
        }
        if end <= self.transmission_range.start {
            return Err(OpossumError::Other(
                "transmission-range end must be greater than start!".into(),
            ));
        }

        self.transmission_range.end = end;
        Ok(())
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<BandFilter> for Spectrum {
    fn from(band_filter: BandFilter) -> Self {
        let mut spectrum =
            Self::new(band_filter.range().clone(), band_filter.resolution()).unwrap();
        let mut spectrum_data = spectrum.data().clone();
        let center_wavelength_in_um = band_filter.center_wavelength().get::<micrometer>();
        let width_in_um = band_filter.width().get::<micrometer>();
        let (in_band, out_of_band) = match band_filter.band_filter_type() {
            BandFilterType::BandPass => (
                band_filter.transmission_range().end,
                band_filter.transmission_range().start,
            ),
            BandFilterType::Notch => (
                band_filter.transmission_range().start,
                band_filter.transmission_range().end,
            ),
        };
        if let Some(smooth_width) = band_filter.smooth_step_width() {
            let mut smooth_width_in_um = smooth_width.get::<micrometer>();
            if smooth_width_in_um > width_in_um {
                warn!(
                    "Smoothing width is larger than actual filter width! Resetting to maximum smoothing width"
                );
                smooth_width_in_um = width_in_um;
            }
            let half_band = width_in_um / 2.0;
            let transition = smooth_width_in_um / 2.0;
            // let lower_start = -half_band - transition;
            // let lower_end = -half_band + transition;
            // let upper_start = half_band - transition;
            // let upper_end = half_band + transition;
            // let transmission_diff =
            //     band_filter.transmission_range().end - band_filter.transmission_range().start;
            spectrum
                .set_data(
                    spectrum_data
                        .iter_mut()
                        .map(|(lambda, _)| {
                            let wvl_diff = *lambda - center_wavelength_in_um;
                            let abs_diff = wvl_diff.abs();

                            let amp = if abs_diff <= (half_band - transition) {
                                in_band // Voll im Durchlassbereich
                            } else if abs_diff >= (half_band + transition) {
                                out_of_band // Sicher im Sperrbereich
                            } else {
                                // Wir befinden uns in einer der beiden Übergangszonen
                                // Normierung des Abstands zur Kante auf [0, 1]
                                let x = (abs_diff - (half_band - transition)) / (2.0 * transition);
                                // Je nach Filtertyp (Notch/Pass) von in_band nach out_of_band oder umgekehrt
                                interpolate_transition(x, in_band, out_of_band)
                            };
                            (*lambda, amp)
                        })
                        .collect(),
                )
                .unwrap();
        } else {
            spectrum
                .set_data(
                    spectrum_data
                        .iter_mut()
                        .map(|(lambda, _)| {
                            if (*lambda - center_wavelength_in_um).abs() >= width_in_um / 2. {
                                (*lambda, out_of_band)
                            } else {
                                (*lambda, in_band)
                            }
                        })
                        .collect::<Vec<(f64, f64)>>(),
                )
                .unwrap();
        }
        spectrum
    }
}
