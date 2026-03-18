use std::fmt::Display;
use std::path::PathBuf;

use crate::error::OpmResult;
use crate::prelude::FilterTypeBuilder;
use crate::spectrum::Spectrum;
use crate::utils::default_from_name::DefaultFromName;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

use super::band::BandFilter;
use super::edge::EdgeFilter;

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

impl From<SpectralFilterBuilder> for FilterTypeBuilder {
    fn from(val: SpectralFilterBuilder) -> Self {
        Self::Spectrum(val)
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
