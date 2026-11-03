//! Builder for geometric light data generation.
//!
//! This module provides the [`RayDataBuilder`] struct, which acts as a wrapper
//! around a [`RayDataSource`]. It handles the generation of rays and applies
//! global properties such as spatial transformations (isometry) and alignment
//! wavelengths to the entire emitted light field.

use crate::{
    error::OpmResult,
    light::Rays,
    prelude::{Isometry, RayDataSource},
};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// Builder struct for generating geometric [`Rays`].
///
/// This struct combines a specific [`RayDataSource`] (e.g., collimated, point source, or image)
/// with global physical properties that apply to the entire generated light field.
/// This includes its 3D position and orientation in the optical system ([`Isometry`]),
/// as well as an optional alignment wavelength used by analyzers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RayDataBuilder {
    source: RayDataSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    isometry: Option<Isometry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alignment_wavelength: Option<Length>,
}

impl Default for RayDataBuilder {
    /// Creates a default [`RayDataBuilder`].
    ///
    /// The default builder uses the default [`RayDataSource`], with no isometry
    /// (placed at the origin without rotation) and no specific alignment wavelength.
    fn default() -> Self {
        Self {
            source: RayDataSource::default(),
            isometry: None,
            alignment_wavelength: None,
        }
    }
}

impl RayDataBuilder {
    /// Builds the rays and automatically applies the global source isometry.
    ///
    /// This method first generates the raw rays using the underlying [`RayDataSource`].
    /// If an [`Isometry`] is defined for this builder, it is then applied to transform
    /// all generated rays into their correct global position and orientation.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying [`RayDataSource`] fails to build the rays
    /// (e.g., due to invalid distributions or file paths).
    pub fn build(&self) -> OpmResult<Rays> {
        // 1. Build the raw rays from the specific source type
        let mut rays = self.source.clone().build()?;

        // 2. Apply the isometry if it is defined
        if let Some(iso) = &self.isometry {
            rays = rays.transformed_by_iso(iso);
        }
        Ok(rays)
    }

    /// Sets the underlying [`RayDataSource`].
    ///
    /// # Arguments
    ///
    /// * `source` - The new ray data source to use for ray generation.
    pub fn set_source(&mut self, source: RayDataSource) {
        self.source = source;
    }
    /// Returns a reference to the underlying [`RayDataSource`].
    #[must_use]
    pub const fn source(&self) -> &RayDataSource {
        &self.source
    }

    /// Sets the global [`Isometry`] (position and orientation) of the generated light field.
    ///
    /// # Arguments
    ///
    /// * `isometry` - The new isometry, or `None` to generate rays at the local origin.
    pub const fn set_isometry(&mut self, isometry: Option<Isometry>) {
        self.isometry = isometry;
    }

    /// Returns the currently defined global [`Isometry`], if any.
    #[must_use]
    pub const fn isometry(&self) -> Option<Isometry> {
        self.isometry
    }

    /// Sets the alignment wavelength.
    ///
    /// This wavelength is primarily used by analyzers (e.g., ray tracing) to determine
    /// the main optical axis of the system.
    ///
    /// # Arguments
    ///
    /// * `alignment_wavelength` - The specific wavelength to use for alignment, or `None`.
    pub fn set_alignment_wavelength(&mut self, alignment_wavelength: Option<Length>) {
        self.alignment_wavelength = alignment_wavelength;
    }

    /// Returns the currently defined alignment wavelength, if any.
    #[must_use]
    pub fn alignment_wavelength(&self) -> Option<Length> {
        self.alignment_wavelength
    }

    /// Returns the wavelength the optical axis of this source is placed at.
    ///
    /// # Returns
    ///
    /// The alignment wavelength, or the central wavelength of the source if none is set (see
    /// [`RayDataSource::central_wavelength`]).
    ///
    /// # Errors
    ///
    /// This function returns an error if no alignment wavelength is set and the central
    /// wavelength of the source cannot be determined.
    pub fn axis_wavelength(&self) -> OpmResult<Length> {
        self.alignment_wavelength
            .map_or_else(|| self.source.central_wavelength(), Ok)
    }
}

impl From<RayDataSource> for RayDataBuilder {
    /// Creates a new [`RayDataBuilder`] from a given [`RayDataSource`].
    ///
    /// The resulting builder will wrap the provided source but will have no
    /// isometry and no alignment wavelength set initially.
    fn from(value: RayDataSource) -> Self {
        let mut rdb = Self::default();
        rdb.set_source(value);
        rdb
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        distributions::spectral::LaserLines, light::lightdata::ray_data_source::CollimatedSrc,
        nanometer,
    };
    use approx::assert_relative_eq;
    use uom::si::length::nanometer;

    /// The axis follows the alignment wavelength where one is set, else the source's own center.
    #[test]
    fn the_axis_wavelength_is_the_alignment_wavelength_or_the_central_one() -> OpmResult<()> {
        let mut source = CollimatedSrc::default();
        source.set_spect_dist(
            LaserLines::new(vec![(nanometer!(500.0), 1.0), (nanometer!(800.0), 3.0)])?.into(),
        );
        let mut builder = RayDataBuilder::from(RayDataSource::Collimated(source));
        assert_relative_eq!(
            builder.axis_wavelength()?.get::<nanometer>(),
            725.0,
            epsilon = 1e-9
        );

        builder.set_alignment_wavelength(Some(nanometer!(1053.0)));
        assert_eq!(builder.axis_wavelength()?, nanometer!(1053.0));

        let empty = RayDataBuilder::from(RayDataSource::Raw(Rays::default()));
        assert!(empty.axis_wavelength().is_err());
        Ok(())
    }
}
