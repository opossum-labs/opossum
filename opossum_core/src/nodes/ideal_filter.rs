#![warn(missing_docs)]
//! ideal filter node
use super::node_attr::NodeAttr;
use crate::{
    analyzers::{
        GhostFocusConfig, RayTraceConfig, energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    error::{OpmResult, OpossumError},
    light_result::{LightRays, LightResult},
    lightdata::LightData,
    micrometer, nanometer,
    optic_node::OpticNode,
    optic_ports::PortType,
    properties::{Proptype, validator::Validator},
    rays::Rays,
    spectrum::Spectrum,
    utils::default_from_name::DefaultFromName,
};
use log::warn;
use num::Zero;
use opm_macros_lib::OpmNode;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, ops::Range, path::PathBuf, str::FromStr};
use strum::{EnumIter, IntoEnumIterator};
use uom::si::{f64::Length, length::micrometer};

/// Config data builder for an [`IdealFilter`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter)]
pub enum FilterTypeBuilder {
    /// a fixed (wavelength-independant) transmission value. Must be between 0.0 and 1.0
    Constant(f64),
    /// filter based on given transmission spectrum.
    Spectrum(SpectralFilterBuilder),
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
            Self::Constant(c) => Ok(FilterType::Constant(*c)),
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
                        return Some(Self::Constant(1.0));
                    }
                    Self::Spectrum(_) => return Some(ftb),
                }
            }
        }
        None
    }
}

/// Config data for an [`IdealFilter`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FilterType {
    /// a fixed (wavelength-independant) transmission value. Must be between 0.0 and 1.0
    Constant(f64),
    /// filter based on given transmission spectrum.
    Spectrum(Spectrum),
}

#[derive(OpmNode, Debug, Clone)]
#[opm_node("darkgray")]
/// An ideal filter with given transmission or optical density.
///
/// ## Optical Ports
///   - Inputs
///     - `front`
///   - Outputs
///     - `rear`
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `filter type`
pub struct IdealFilter {
    node_attr: NodeAttr,
}
unsafe impl Send for IdealFilter {}

impl Default for IdealFilter {
    /// Create an ideal filter node with a transmission of 100%.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("ideal filter");
        node_attr
            .create_property_with_validator(
                "filter type builder",
                "used filter algorithm",
                Validator::NumericInRange { min: 0., max: 1. },
                FilterTypeBuilder::Constant(1.0).into(),
            )
            .unwrap();
        let mut idf = Self { node_attr };
        idf.update_surfaces().unwrap();
        idf
    }
}
impl IdealFilter {
    /// Creates a new [`IdealFilter`] with a given [`FilterType`].
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError::Other`] if the filter type is
    /// [`FilterType::Constant`] and the transmission factor is outside the interval [0.0; 1.0].
    pub fn new(name: &str, filter_type_builder: &FilterTypeBuilder) -> OpmResult<Self> {
        let mut filter = Self::default();
        filter
            .node_attr
            .set_property("filter type builder", filter_type_builder.clone().into())?;
        filter.node_attr.set_name(name);
        Ok(filter)
    }
    /// Returns the filter type of this [`IdealFilter`].
    ///
    /// # Errors
    /// Errors if the wrong data type is stored in the filter-type properties
    pub fn filter_type(&self) -> OpmResult<FilterType> {
        if let Proptype::FilterTypeBuilder(filter_type_builder) =
            self.node_attr.get_property("filter type builder")?
        {
            filter_type_builder.build()
        } else {
            Err(OpossumError::Properties(
                "Property: `filter type builder` not found".into(),
            ))
        }
    }
    /// Sets a constant transmission value for this [`IdealFilter`].
    ///
    /// This implicitly sets the filter type to [`FilterType::Constant`].
    /// # Errors
    ///
    /// This function will return an error if a transmission factor > 1.0 is given (This would be an amplifiying filter :-) ).
    pub fn set_transmission(&mut self, transmission: f64) -> OpmResult<()> {
        if (0.0..=1.0).contains(&transmission) {
            self.node_attr.set_property(
                "filter type builder",
                FilterTypeBuilder::Constant(transmission).into(),
            )?;
            Ok(())
        } else {
            Err(OpossumError::Other(
                "attenuation must be in interval [0.0; 1.0]".into(),
            ))
        }
    }
    /// Sets the transmission of this [`IdealFilter`] expressed as optical density.
    ///
    /// This implicitly sets the filter type to [`FilterType::Constant`].
    /// # Errors
    ///
    /// This function will return an error if an optical density < 0.0 was given.
    pub fn set_optical_density(&mut self, density: f64) -> OpmResult<()> {
        if density >= 0.0 {
            self.node_attr.set_property(
                "filter type builder",
                FilterTypeBuilder::Constant(f64::powf(10.0, -density)).into(),
            )?;
            Ok(())
        } else {
            Err(OpossumError::Other("optical densitiy must be >=0".into()))
        }
    }
    /// Returns the transmission factor of this [`IdealFilter`] expressed as optical density for the [`FilterType::Constant`].
    ///
    /// This functions `None` if the filter type is not [`FilterType::Constant`].
    #[must_use]
    pub fn optical_density(&self) -> Option<f64> {
        self.filter_type()
            .map_or(None, |filter_type| match filter_type {
                FilterType::Constant(t) => Some(-f64::log10(t)),
                FilterType::Spectrum(_) => None,
            })
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
        let (before_edge_wvl, after_edge_wvl, angle_sign) = match self.edge_filter_type() {
            EdgeFilterType::LongPass => (
                self.transmission_range.start,
                self.transmission_range.end,
                1.,
            ),
            EdgeFilterType::ShortPass => (
                self.transmission_range.end,
                self.transmission_range.start,
                -1.,
            ),
        };

        self.smooth_step_width().map_or_else(
            || self.step_transmission(wavelength, before_edge_wvl, after_edge_wvl),
            |width| {
                self.smooth_step_transmission(
                    wavelength,
                    width,
                    before_edge_wvl,
                    after_edge_wvl,
                    angle_sign,
                )
            },
        )
    }

    fn smooth_step_transmission(
        &self,
        wavelength: Length,
        width: Length,
        before_edge_wvl: f64,
        after_edge_wvl: f64,
        angle_sign: f64,
    ) -> f64 {
        let transmission_diff = self.transmission_range.end - self.transmission_range.start;
        let wvl_diff = wavelength - self.edge_wavelength();
        if wvl_diff <= -width / 2.0 {
            before_edge_wvl
        } else if wvl_diff > width / 2.0 {
            after_edge_wvl
        } else {
            let angle = (std::f64::consts::PI / width * wvl_diff).value;
            (0.5 * transmission_diff).mul_add(
                angle_sign * angle.sin(),
                0.5f64.mul_add(transmission_diff, self.transmission_range().start),
            )
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

        let (before_edge_wvl, after_edge_wvl, angle_sign) = match edge_filter.edge_filter_type() {
            EdgeFilterType::LongPass => (
                edge_filter.transmission_range().start,
                edge_filter.transmission_range().end,
                1.,
            ),
            EdgeFilterType::ShortPass => (
                edge_filter.transmission_range().end,
                edge_filter.transmission_range().start,
                -1.,
            ),
        };
        if let Some(width) = edge_filter.smooth_step_width() {
            spectrum.map_mut(|(lambda, _)| {
                (
                    *lambda,
                    edge_filter.smooth_step_transmission(
                        micrometer!(*lambda),
                        width,
                        before_edge_wvl,
                        after_edge_wvl,
                        angle_sign,
                    ),
                )
            });
        } else {
            spectrum.map_mut(|(lambda, _)| {
                (
                    *lambda,
                    edge_filter.step_transmission(
                        micrometer!(*lambda),
                        before_edge_wvl,
                        after_edge_wvl,
                    ),
                )
            });
        }
        spectrum
    }
}

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

        let center_wavelength_in_um = band_filter.center_wavelength().get::<micrometer>();
        let width_in_um = band_filter.width().get::<micrometer>();
        let (in_band, out_of_band, angle_sign) = match band_filter.band_filter_type() {
            BandFilterType::BandPass => (
                band_filter.transmission_range().end,
                band_filter.transmission_range().start,
                -1.,
            ),
            BandFilterType::Notch => (
                band_filter.transmission_range().start,
                band_filter.transmission_range().end,
                1.,
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
            let lower_start = -half_band - transition;
            let lower_end = -half_band + transition;
            let upper_start = half_band - transition;
            let upper_end = half_band + transition;
            let transmission_diff =
                band_filter.transmission_range().end - band_filter.transmission_range().start;
            spectrum.map_mut(|(lambda, _)| {
                let wvl_diff = *lambda - band_filter.center_wavelength().get::<micrometer>();

                let amp = if wvl_diff <= lower_start || wvl_diff >= upper_end {
                    out_of_band
                } else if wvl_diff >= lower_end && wvl_diff <= upper_start {
                    in_band
                } else if wvl_diff > lower_start && wvl_diff < lower_end {
                    // Lower transition
                    let x = (wvl_diff - lower_start) / (2.0 * transition);
                    (angle_sign * 0.5 * transmission_diff).mul_add(
                        (std::f64::consts::PI * x).cos(),
                        0.5f64.mul_add(transmission_diff, band_filter.transmission_range().start),
                    )
                } else {
                    // Upper transition
                    let x = (upper_end - wvl_diff) / (2.0 * transition);
                    (angle_sign * 0.5 * transmission_diff).mul_add(
                        (std::f64::consts::PI * x).cos(),
                        0.5f64.mul_add(transmission_diff, band_filter.transmission_range().start),
                    )
                };
                (*lambda, amp)
            });
        } else {
            spectrum.map_mut(|(lambda, _)| {
                if (*lambda - center_wavelength_in_um).abs() >= width_in_um / 2. {
                    (*lambda, out_of_band)
                } else {
                    (*lambda, in_band)
                }
            });
        }
        spectrum
    }
}

/// Represents different ways to create a spectral filter.
///
/// This enum can hold:
/// - An [`EdgeFilter`] instance for edge-type filters.
/// - A [`BandFilter`] instance for band-pass filters.
/// - A file path for loading a filter from external data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter)]
pub enum SpectralFilterBuilder {
    /// Builds a filter from an [`EdgeFilter`] definition.
    EdgeFilter(EdgeFilter),

    /// Builds a filter from a [`BandFilter`] definition.
    BandFilter(BandFilter),

    /// Builds a filter by loading data from a file at the given path.
    FromFile(PathBuf),
}

impl Default for SpectralFilterBuilder {
    fn default() -> Self {
        Self::BandFilter(BandFilter::default())
    }
}

impl Display for SpectralFilterBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EdgeFilter(_) => write!(f, "Edge filter"),
            Self::BandFilter(_) => write!(f, "Band filter"),
            Self::FromFile(_) => write!(f, "From file "),
        }
    }
}

impl DefaultFromName for SpectralFilterBuilder {}

impl From<BandFilter> for SpectralFilterBuilder {
    fn from(val: BandFilter) -> Self {
        Self::BandFilter(val)
    }
}
impl From<EdgeFilter> for SpectralFilterBuilder {
    fn from(val: EdgeFilter) -> Self {
        Self::EdgeFilter(val)
    }
}

impl From<SpectralFilterBuilder> for FilterTypeBuilder {
    fn from(val: SpectralFilterBuilder) -> Self {
        Self::Spectrum(val)
    }
}

impl From<f64> for FilterTypeBuilder {
    fn from(val: f64) -> Self {
        Self::Constant(val)
    }
}

impl SpectralFilterBuilder {
    /// Constructs a [`Spectrum`] object from the builder.
    ///
    /// # Returns
    /// - A [`Spectrum`] instance corresponding to the variant:
    ///   - `EdgeFilter`: Converts the contained `EdgeFilter` to a spectrum.
    ///   - `BandFilter`: Converts the contained `BandFilter` to a spectrum.
    ///   - `FromFile`: Loads a given csv file and converts it to a spectrum
    /// # Errors
    /// Returns an error if the creation of a spectrum from a .csv fails.
    pub fn build(&self) -> OpmResult<Spectrum> {
        match self {
            Self::EdgeFilter(edge_filter) => Ok(edge_filter.clone().into()),
            Self::BandFilter(band_filter) => Ok(band_filter.clone().into()),
            Self::FromFile(p) => {
                let spec = Spectrum::from_csv(p)?;
                Ok(spec)
            }
        }
    }

    /// Check if the [`Spectrum`] values that will be produced by this [`SpectralFilterBuilder`] are in a specific range.
    ///
    /// This functions checks if all values are in the range (min..=max)
    /// # Errors
    /// This function returns an error if building the spectrum from a file fails
    pub fn values_are_in_range(&self, min: f64, max: f64) -> OpmResult<bool> {
        match self {
            Self::EdgeFilter(edge_filter) => Ok(min <= edge_filter.transmission_range().start
                && max >= edge_filter.transmission_range().end),
            Self::BandFilter(band_filter) => Ok(min <= band_filter.transmission_range().start
                && max >= band_filter.transmission_range().end),
            Self::FromFile(path_buf) => {
                if path_buf.as_os_str().is_empty() {
                    // as of now this can not be checked
                    Ok(true)
                } else {
                    Ok(self.build()?.values_are_in_range(min, max))
                }
            }
        }
    }

    /// Returns the File path of this [`SpectralFilterBuilder`], wrapped into an option if the type matches. Returns None otherwise
    #[must_use]
    pub fn file_path(&self) -> Option<PathBuf> {
        if let Self::FromFile(p) = self {
            Some(p.clone())
        } else {
            None
        }
    }
}

impl OpticNode for IdealFilter {
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn set_apodization_warning(&mut self, _apodized: bool) {}
    fn reset_data(&mut self) {
        self.reset_optic_surfaces();
    }
}
impl AnalysisGhostFocus for IdealFilter {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let filter_type = self.filter_type()?;
        let mut output =
            AnalysisGhostFocus::analyze_single_surface_node(self, incoming_data, config)?;
        let out_port = &self.ports().names(&PortType::Output)[0];
        if let Some(rays_bundles) = output.get_mut(out_port) {
            for rays in rays_bundles {
                rays.filter_energy(&filter_type)?;
            }
            Ok(output)
        } else {
            Err(OpossumError::Analysis("filtering of rays failed".into()))
        }
    }
}
impl AnalysisEnergy for IdealFilter {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let filter_type = self.filter_type()?;
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(input) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        if let LightData::Energy(s) = input {
            let mut new_spectrum = s.clone();
            new_spectrum.filter_with_type(&filter_type)?;
            let light_data = LightData::Energy(new_spectrum);
            Ok(LightResult::from([(out_port.into(), light_data)]))
        } else {
            Err(OpossumError::Analysis("expected energy light data".into()))
        }
    }
}
impl AnalysisRayTrace for IdealFilter {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let filter_type = self.filter_type()?;

        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(input) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        let LightData::Geometric(r) = input else {
            return Err(OpossumError::Analysis(
                "expected geometric light data".into(),
            ));
        };
        let mut rays = r.clone();
        let iso = self.effective_surface_iso(in_port)?;
        let Some(surf) = self.get_optic_surface_mut(in_port) else {
            return Err(OpossumError::Analysis("no surface found. Aborting".into()));
        };
        let refraction_intended = true;
        rays.refract_on_surface(
            surf,
            None,
            refraction_intended,
            config.missed_surface_strategy(),
        )?;
        rays.filter_energy(&filter_type)?;
        match self.ports().aperture(&PortType::Input, in_port) {
            Some(aperture) => {
                rays.apodize(aperture, &iso)?;
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
            _ => {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            }
        }
        match self.ports().aperture(&PortType::Output, out_port) {
            Some(aperture) => {
                rays.apodize(aperture, &iso)?;
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
            _ => {
                return Err(OpossumError::OpticPort("output aperture not found".into()));
            }
        }
        let light_data = LightData::Geometric(rays);
        Ok(LightResult::from([(out_port.into(), light_data)]))
    }
}

#[cfg(test)]
mod test {
    use approx::assert_abs_diff_eq;
    use uom::si::energy::joule;

    use crate::{
        analyzers::RayTraceConfig, joule, lightdata::LightData, millimeter, nanometer,
        nodes::test_helper::test_helper::*, optic_ports::PortType,
        position_distributions::Hexapolar, rays::Rays, spectrum_helper::create_he_ne_spec,
        utils::geom_transformation::Isometry,
    };
    use crate::{micrometer, utils::test_helper::test_helper::check_logs};
    use num::Zero;
    use testing_logger;

    use super::*;
    #[test]
    fn default() {
        let mut node = IdealFilter::default();
        assert_eq!(node.filter_type().unwrap(), FilterType::Constant(1.0));
        assert_eq!(node.name(), "ideal filter");
        assert_eq!(node.node_type(), "ideal filter");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "darkgray");
        assert!(node.as_group_mut().is_err());
    }
    #[test]
    fn new() {
        assert!(IdealFilter::new("test", &FilterTypeBuilder::Constant(1.1)).is_err());
        assert!(IdealFilter::new("test", &FilterTypeBuilder::Constant(-0.1)).is_err());
        let node = IdealFilter::new("test", &FilterTypeBuilder::Constant(0.8)).unwrap();
        assert_eq!(node.name(), "test");
        assert_eq!(node.filter_type().unwrap(), FilterType::Constant(0.8));
    }
    #[test]
    fn set_transmission() {
        let mut node = IdealFilter::default();
        assert!(node.set_transmission(-0.1).is_err());
        assert!(node.set_transmission(1.1).is_err());
        assert!(node.set_transmission(0.5).is_ok());
        assert_eq!(node.filter_type().unwrap(), FilterType::Constant(0.5));
    }
    #[test]
    fn optical_density() {
        let mut node = IdealFilter::default();
        assert_eq!(node.optical_density(), Some(0.0));
        node.set_transmission(0.1).unwrap();
        assert_eq!(node.optical_density(), Some(1.0));
        node.set_transmission(0.01).unwrap();
        assert_eq!(node.optical_density(), Some(2.0));
        let node = IdealFilter::new(
            "test",
            &FilterTypeBuilder::Spectrum(BandFilter::default().into()),
        )
        .unwrap();
        assert_eq!(node.optical_density(), None);
    }
    #[test]
    fn set_optical_density() {
        let mut node = IdealFilter::default();
        assert!(node.set_optical_density(-1.0).is_err());
        assert!(node.set_optical_density(1.0).is_ok());
        assert_eq!(node.filter_type().unwrap(), FilterType::Constant(0.1));
        assert!(node.set_optical_density(f64::NAN).is_err());
        assert!(node.set_optical_density(f64::INFINITY).is_ok());
        assert_eq!(node.filter_type().unwrap(), FilterType::Constant(0.0));
    }
    #[test]
    fn inverted() {
        test_inverted::<IdealFilter>()
    }
    #[test]
    fn ports() {
        let node = IdealFilter::default();
        assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut node = IdealFilter::default();
        node.set_inverted(true).unwrap();
        assert_eq!(node.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["input_1"]);
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<IdealFilter>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = IdealFilter::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_geometric_wrong_data_type() {
        test_analyze_wrong_data_type::<IdealFilter>("input_1");
    }
    #[test]
    fn analyze_energy_ok() {
        let mut node = IdealFilter::new("test", &FilterTypeBuilder::Constant(0.5)).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("input_1".into(), input_light.clone());
        assert!(
            AnalysisRayTrace::analyze(&mut node, input.clone(), &RayTraceConfig::default())
                .is_err()
        );
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        let expected_output_light = LightData::Energy(create_he_ne_spec(0.5).unwrap());
        assert_eq!(*output, expected_output_light);
    }
    #[test]
    fn analyzer_geometric_fixed() {
        let mut node = IdealFilter::new("test", &FilterTypeBuilder::Constant(0.3)).unwrap();
        node.set_isometry(Isometry::identity()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(
            Rays::new_uniform_collimated(
                nanometer!(1054.0),
                joule!(1.0),
                &Hexapolar::new(millimeter!(5.0), 1).unwrap(),
            )
            .unwrap(),
        );
        input.insert("input_1".into(), input_light.clone());
        assert!(AnalysisEnergy::analyze(&mut node, input.clone()).is_err());
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        if let LightData::Geometric(output) = output.clone().unwrap() {
            assert_abs_diff_eq!(output.total_energy().get::<joule>(), 0.3);
        } else {
            panic!("wrong data LightData format")
        }
    }
    #[test]
    fn analyze_inverse() {
        let mut node = IdealFilter::new("test", &FilterTypeBuilder::Constant(0.5)).unwrap();
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        let expected_output_light = LightData::Energy(create_he_ne_spec(0.5).unwrap());
        assert_eq!(*output, expected_output_light);
    }

    #[test]
    fn test_short_pass_filter() {
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
        )
        .unwrap()
        .into();

        assert_eq!(s.get_value(&micrometer!(1.0)).unwrap(), 1.0);
        assert_eq!(s.get_value(&micrometer!(2.0)).unwrap(), 1.0);
        assert_eq!(s.get_value(&micrometer!(3.0)).unwrap(), 1.0);
        assert_eq!(s.get_value(&micrometer!(4.0)).unwrap(), 0.0);
    }

    #[test]
    fn test_long_pass_filter() {
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
        )
        .unwrap()
        .into();

        assert_eq!(s.get_value(&micrometer!(1.0)).unwrap(), 0.0);
        assert_eq!(s.get_value(&micrometer!(2.0)).unwrap(), 0.0);
        assert_eq!(s.get_value(&micrometer!(3.0)).unwrap(), 0.0);
        assert_eq!(s.get_value(&micrometer!(4.0)).unwrap(), 1.0);
    }
    #[test]
    fn test_short_pass_smooth_filter() {
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
            range.clone(),
            resolution,
        )
        .unwrap()
        .into();

        assert_eq!(s.get_value(&micrometer!(1.0)).unwrap(), 1.0);
        assert_eq!(s.get_value(&micrometer!(2.0)).unwrap(), 1.0);
        assert_eq!(s.get_value(&micrometer!(2.5)).unwrap(), 1.0);
        assert_eq!(s.get_value(&micrometer!(3.0)).unwrap(), 0.5);
        assert_eq!(s.get_value(&micrometer!(3.5)).unwrap(), 0.0);
        assert_eq!(s.get_value(&micrometer!(4.0)).unwrap(), 0.0);
    }
}
