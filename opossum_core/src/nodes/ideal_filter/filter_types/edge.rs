use std::{fmt::Display, ops::Range, str::FromStr};

use log::warn;
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use strum::EnumIter;
use uom::si::f64::Length;

use crate::{
    error::{OpmResult, OpossumError},
    light::Spectrum,
    micrometer, nanometer,
    nodes::ideal_filter::filter_types::math::interpolate_transition,
    prelude::SpectralFilterBuilder,
    utils::default_from_name::DefaultFromName,
};

/// Represents an optical edge filter with defined characteristics.
///
/// This struct stores the edge filter type, the edge wavelength,
/// an optional smooth transition width, the operational wavelength range,
/// and the resolution of the filter data.
///
/// # Note
/// The edge wavelength is included in a short-pass and excluded ind the long-pass filter
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdgeFilter {
    /// The type of edge filter (long‑pass or short‑pass).
    filter_type: EdgeFilterType,

    /// The cut-on / cut-off (edge) wavelength of the filter.
    /// The edge wavelength is included in a short-pass and excluded ind the long-pass filter
    edge_wavelength: Length,

    /// Minimum and maximum values of this filters' transmission
    transmission_range: Range<f64>,
    /// The optional smooth transition width at the edge wavelength.
    ///
    /// If `Some`, this specifies the width of a gradual transition;
    /// if `None`, the filter is assumed to have a sharp edge.
    smooth_step_width: Option<Length>,

    /// The wavelength range over which the filter is defined.
    range: Range<Length>,

    /// The wavelength resolution associated with the filter's data.
    resolution: Length,
}
impl Default for EdgeFilter {
    fn default() -> Self {
        Self {
            filter_type: EdgeFilterType::ShortPass,
            edge_wavelength: nanometer!(1000.),
            transmission_range: (0.)..1.,
            smooth_step_width: Some(nanometer!(2.)),
            range: nanometer!(900.)..nanometer!(1100.),
            resolution: nanometer!(0.2),
        }
    }
}
impl EdgeFilter {
    /// Creates a new `EdgeFilter` instance.
    ///
    /// # Parameters
    /// - `edge_filter_type`: The type of the edge filter.
    /// - `edge_wavelength`: The edge wavelength. Must be positive and finite.
    /// - `smooth_step_width`: Optional step width. If provided, must be positive and finite.
    /// - `range`: The wavelength range. Start and end must be positive, finite, and `end` must be greater than `start`.
    /// - `resolution`: The resolution. Must be positive and finite.
    ///
    /// # Returns
    /// A new `EdgeFilter` instance wrapped in `Ok` if all parameters are valid.
    ///
    /// # Errors
    /// Returns an error if any provided parameter is invalid.
    pub fn new(
        edge_filter_type: EdgeFilterType,
        edge_wavelength: Length,
        transmission_range: Range<f64>,
        smooth_step_width: Option<Length>,
        range: Range<Length>,
        resolution: Length,
    ) -> OpmResult<Self> {
        if !edge_wavelength.is_normal() || edge_wavelength.is_sign_negative() {
            return Err(OpossumError::Other(
                "Edge wavelength must be positive and finite!".into(),
            ));
        }
        if let Some(width) = smooth_step_width
            && (!width.is_normal() || width.is_sign_negative())
        {
            return Err(OpossumError::Other(
                "Step width must be positive and finite when provided!".into(),
            ));
        }
        if !resolution.is_normal() || resolution.is_sign_negative() {
            return Err(OpossumError::Other(
                "Resolution must be positive and finite!".into(),
            ));
        }
        if !range.start.is_normal() || range.start.is_sign_negative() {
            return Err(OpossumError::Other(
                "Range start must be positive and finite!".into(),
            ));
        }
        if !range.end.is_normal() || range.end.is_sign_negative() || range.end <= range.start {
            return Err(OpossumError::Other(
                "Range end must be positive, finite, and greater than start!".into(),
            ));
        }
        if !range.contains(&edge_wavelength) {
            warn!("cut-off / cut-on wavelength must be inside the spectrum range");
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

        Ok(Self {
            filter_type: edge_filter_type,
            edge_wavelength,
            transmission_range,
            smooth_step_width,
            range,
            resolution,
        })
    }

    /// Returns the edge filter type.
    #[must_use]
    pub const fn edge_filter_type(&self) -> &EdgeFilterType {
        &self.filter_type
    }

    /// Sets the edge filter type.
    ///
    /// # Parameters
    /// - `edge_filter_type`: The new filter type.
    pub const fn set_edge_filter_type(&mut self, edge_filter_type: EdgeFilterType) {
        self.filter_type = edge_filter_type;
    }

    /// Returns the edge wavelength.
    #[must_use]
    pub fn edge_wavelength(&self) -> Length {
        self.edge_wavelength
    }

    /// Sets the edge wavelength.
    ///
    /// # Parameters
    /// - `edge_wavelength`: The new edge wavelength.
    ///
    /// # Errors
    /// Returns an error if the value is not positive and finite.
    pub fn set_edge_wavelength(&mut self, edge_wavelength: Length) -> OpmResult<()> {
        if !edge_wavelength.is_normal() || edge_wavelength.is_sign_negative() {
            return Err(OpossumError::Other(
                "Edge wavelength must be positive and finite!".into(),
            ));
        }
        self.edge_wavelength = edge_wavelength;
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
    pub fn set_smooth_step_width(&mut self, step_width: Option<Length>) -> OpmResult<()> {
        if let Some(width) = step_width
            && (!width.is_normal() || width.is_sign_negative())
        {
            return Err(OpossumError::Other(
                "Step width must be positive and finite when provided!".into(),
            ));
        }
        self.smooth_step_width = step_width;
        Ok(())
    }

    /// Returns the wavelength range.
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
    /// Returns an error if the range is invalid.
    pub fn set_range(&mut self, range: Range<Length>) -> OpmResult<()> {
        if !range.start.is_normal() || range.start.is_sign_negative() {
            return Err(OpossumError::Other(
                "Range start must be positive and finite!".into(),
            ));
        }
        if !range.end.is_normal() || range.end.is_sign_negative() || range.end <= range.start {
            return Err(OpossumError::Other(
                "Range end must be positive, finite, and greater than start!".into(),
            ));
        }
        self.range = range;
        Ok(())
    }

    /// Returns the resolution.
    #[must_use]
    pub fn resolution(&self) -> Length {
        self.resolution
    }

    /// Sets the resolution.
    ///
    /// # Parameters
    /// - `resolution`: The new resolution.
    ///
    /// # Errors
    /// Returns an error if the value is not positive and finite.
    pub fn set_resolution(&mut self, resolution: Length) -> OpmResult<()> {
        if !resolution.is_normal() || resolution.is_sign_negative() {
            return Err(OpossumError::Other(
                "Resolution must be positive and finite!".into(),
            ));
        }
        self.resolution = resolution;
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
    pub fn set_transmission_range(&mut self, transmission: Range<f64>) -> OpmResult<()> {
        if transmission.start > 1. || transmission.end.is_sign_negative() {
            return Err(OpossumError::Other("Transmission minimum of Band-Filter must be positive, smaller than 1. and greater than 0!".into()));
        }
        if transmission.end > 1.
            || transmission.end.is_sign_negative()
            || transmission.end <= transmission.start
        {
            return Err(OpossumError::Other("Transmission maximum of Band-Filter must be positive, smaller than 1., greater than 0 and greater than the transmission minimum!".into()));
        }
        self.transmission_range = transmission;
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

    /// Calculates the transmission value of the edge filter at a given wavelength.
    ///
    /// # Parameters
    /// - `wavelength`: The wavelength at which to compute the transmission.
    ///
    /// # Returns
    /// A floating-point value (`f64`) representing the transmission
    ///
    /// # Behavior
    /// - If `smooth_step_width` is defined, the function uses a smooth transition
    ///   function (`smooth_step_transmission`).
    /// - Otherwise, it uses a sharp step function (`step_transmission`).
    #[must_use]
    pub fn transmission(&self, wavelength: Length) -> f64 {
        let (before_edge_wvl, after_edge_wvl) = match self.edge_filter_type() {
            EdgeFilterType::LongPass => {
                (self.transmission_range.start, self.transmission_range.end)
            }
            EdgeFilterType::ShortPass => {
                (self.transmission_range.end, self.transmission_range.start)
            }
        };

        self.smooth_step_width().map_or_else(
            || self.step_transmission(wavelength, before_edge_wvl, after_edge_wvl),
            |width| {
                self.smooth_step_transmission(wavelength, width, before_edge_wvl, after_edge_wvl)
            },
        )
    }

    fn smooth_step_transmission(
        &self,
        wavelength: Length,
        width: Length,
        before_edge_wvl: f64,
        after_edge_wvl: f64,
    ) -> f64 {
        let wvl_diff = wavelength - self.edge_wavelength();
        let half_width = width / 2.0;

        if wvl_diff <= -half_width {
            before_edge_wvl
        } else if wvl_diff >= half_width {
            after_edge_wvl
        } else {
            let x = (wvl_diff + half_width).value / width.value;
            interpolate_transition(x, before_edge_wvl, after_edge_wvl)
        }
    }

    fn step_transmission(
        &self,
        wavelength: Length,
        before_edge_wvl: f64,
        after_edge_wvl: f64,
    ) -> f64 {
        if wavelength - self.edge_wavelength() > Length::zero() {
            after_edge_wvl
        } else {
            before_edge_wvl
        }
    }
}
#[allow(clippy::fallible_impl_from)]
impl From<EdgeFilter> for Spectrum {
    fn from(edge_filter: EdgeFilter) -> Self {
        let mut spectrum =
            Self::new(edge_filter.range().clone(), edge_filter.resolution()).unwrap();
        let mut spectrum_data = spectrum.data().clone();
        let (before_edge_wvl, after_edge_wvl) = match edge_filter.edge_filter_type() {
            EdgeFilterType::LongPass => (
                edge_filter.transmission_range().start,
                edge_filter.transmission_range().end,
            ),
            EdgeFilterType::ShortPass => (
                edge_filter.transmission_range().end,
                edge_filter.transmission_range().start,
            ),
        };
        if let Some(width) = edge_filter.smooth_step_width() {
            spectrum
                .set_data(
                    spectrum_data
                        .iter_mut()
                        .map(|(lambda, _)| {
                            (
                                *lambda,
                                edge_filter.smooth_step_transmission(
                                    micrometer!(*lambda),
                                    width,
                                    before_edge_wvl,
                                    after_edge_wvl,
                                ),
                            )
                        })
                        .collect::<Vec<(f64, f64)>>(),
                )
                .unwrap();
        } else {
            spectrum
                .set_data(
                    spectrum_data
                        .iter_mut()
                        .map(|(lambda, _)| {
                            (
                                *lambda,
                                edge_filter.step_transmission(
                                    micrometer!(*lambda),
                                    before_edge_wvl,
                                    after_edge_wvl,
                                ),
                            )
                        })
                        .collect::<Vec<(f64, f64)>>(),
                )
                .unwrap();
        }
        spectrum
    }
}

/// Specifies the type of edge filter.
///
/// - `LongPass`: Allows wavelengths longer than the specified edge wavelength to pass.
/// - `ShortPass`: Allows wavelengths shorter than the specified edge wavelength to pass.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter, Eq, Copy, Default)]
pub enum EdgeFilterType {
    /// Passes wavelengths longer than the edge wavelength.
    LongPass,

    /// Passes wavelengths shorter than the edge wavelength.
    #[default]
    ShortPass,
}

impl Display for EdgeFilterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LongPass => write!(f, "Long pass"),
            Self::ShortPass => write!(f, "Short pass"),
        }
    }
}

impl DefaultFromName for EdgeFilterType {}

impl FromStr for EdgeFilterType {
    type Err = OpossumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::default_from_name(s).map_or_else(
            || {
                Err(OpossumError::Other(
                    "Invalid str identifier to create EdgeFilterType from string!".into(),
                ))
            },
            Ok,
        )
    }
}
impl From<EdgeFilter> for SpectralFilterBuilder {
    fn from(val: EdgeFilter) -> Self {
        Self::EdgeFilter(val)
    }
}
#[cfg(test)]
mod test {
    use crate::utils::test_helper::test_helper::check_logs;

    use super::*;
    #[test]
    fn test_short_pass_filter() -> OpmResult<()> {
        testing_logger::setup();
        assert!(
            EdgeFilter::new(
                EdgeFilterType::ShortPass,
                micrometer!(7.0),
                (0.)..(1.),
                None,
                micrometer!(1.0)..micrometer!(5.0),
                micrometer!(1.0)
            )
            .is_ok()
        );
        check_logs(
            log::Level::Warn,
            vec!["cut-off / cut-on wavelength must be inside the spectrum range"],
        );
        let s: Spectrum = EdgeFilter::new(
            EdgeFilterType::ShortPass,
            micrometer!(3.0),
            (0.)..(1.),
            None,
            micrometer!(1.0)..micrometer!(5.0),
            micrometer!(1.0),
        )?
        .into();

        assert_eq!(s.get_value(&micrometer!(1.0)), Some(1.0));
        assert_eq!(s.get_value(&micrometer!(2.0)), Some(1.0));
        assert_eq!(s.get_value(&micrometer!(3.0)), Some(1.0));
        assert_eq!(s.get_value(&micrometer!(4.0)), Some(0.0));
        Ok(())
    }

    #[test]
    fn test_long_pass_filter() -> OpmResult<()> {
        testing_logger::setup();
        assert!(
            EdgeFilter::new(
                EdgeFilterType::LongPass,
                micrometer!(7.0),
                (0.)..(1.),
                None,
                micrometer!(1.0)..micrometer!(5.0),
                micrometer!(1.0)
            )
            .is_ok()
        );
        check_logs(
            log::Level::Warn,
            vec!["cut-off / cut-on wavelength must be inside the spectrum range"],
        );
        let s: Spectrum = EdgeFilter::new(
            EdgeFilterType::LongPass,
            micrometer!(3.0),
            (0.)..(1.),
            None,
            micrometer!(1.0)..micrometer!(5.0),
            micrometer!(1.0),
        )?
        .into();

        assert_eq!(s.get_value(&micrometer!(1.0)), Some(0.0));
        assert_eq!(s.get_value(&micrometer!(2.0)), Some(0.0));
        assert_eq!(s.get_value(&micrometer!(3.0)), Some(0.0));
        assert_eq!(s.get_value(&micrometer!(4.0)), Some(1.0));
        Ok(())
    }
    #[test]
    fn test_short_pass_smooth_filter() -> OpmResult<()> {
        let range = micrometer!(1.0)..micrometer!(5.0);
        let resolution = micrometer!(0.5);
        assert!(
            EdgeFilter::new(
                EdgeFilterType::ShortPass,
                micrometer!(3.0),
                (0.)..(1.),
                Some(Length::zero()),
                range.clone(),
                resolution
            )
            .is_err()
        );
        assert!(
            EdgeFilter::new(
                EdgeFilterType::ShortPass,
                micrometer!(3.0),
                (0.)..(1.),
                Some(micrometer!(-1.0)),
                range.clone(),
                resolution
            )
            .is_err()
        );
        let s: Spectrum = EdgeFilter::new(
            EdgeFilterType::ShortPass,
            micrometer!(3.0),
            (0.)..(1.),
            Some(micrometer!(1.0)),
            range,
            resolution,
        )?
        .into();

        assert_eq!(s.get_value(&micrometer!(1.0)), Some(1.0));
        assert_eq!(s.get_value(&micrometer!(2.0)), Some(1.0));
        assert_eq!(s.get_value(&micrometer!(2.5)), Some(1.0));
        assert_eq!(s.get_value(&micrometer!(3.0)), Some(0.5));
        assert_eq!(s.get_value(&micrometer!(3.5)), Some(0.0));
        assert_eq!(s.get_value(&micrometer!(4.0)), Some(0.0));
        Ok(())
    }
}
