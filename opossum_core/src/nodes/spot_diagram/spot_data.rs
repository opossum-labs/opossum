#![warn(missing_docs)]
//! Spot diagram data structures and plotting routines.

use crate::{error::OpmResult, light::Rays, utils::geom_transformation::Isometry};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// A single spectral series containing 2D spot coordinates at a specific wavelength.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WavelengthSpotSeries {
    /// Nominal wavelength for this spectral bin.
    pub wavelength: Length,
    /// 2D positions (x, y) of ray hits on the detector surface.
    pub spots: Vec<(Length, Length)>,
}

impl WavelengthSpotSeries {
    /// Creates a new spectral spot series.
    #[must_use]
    pub fn new(wavelength: Length, spots: Vec<(Length, Length)>) -> Self {
        Self { wavelength, spots }
    }
}

/// Compact 2D spot diagram data binned by wavelength.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SpotDiagramData {
    /// Spectral series containing ray spots.
    pub series: Vec<WavelengthSpotSeries>,
}

impl SpotDiagramData {
    /// Creates a new [`SpotDiagramData`].
    #[must_use]
    pub const fn new(series: Vec<WavelengthSpotSeries>) -> Self {
        Self { series }
    }

    /// Extracts spot diagram data from a ray bundle, grouped into wavelength bins.
    ///
    /// # Errors
    /// Returns an error if ray splitting by wavelength fails.
    pub fn from_rays(rays: &Rays, iso: &Isometry, wavelength_bin_width: Length) -> OpmResult<Self> {
        if rays.is_empty() {
            return Ok(Self::default());
        }

        let (split_rays_bundles, wavelengths) =
            rays.split_ray_bundle_by_wavelength(wavelength_bin_width, true)?;

        let mut series = Vec::with_capacity(split_rays_bundles.len());
        for (bundle, &wvl) in split_rays_bundles.iter().zip(wavelengths.iter()) {
            let xy_pos = bundle.get_xy_rays_pos(true, iso);
            let mut spots = Vec::with_capacity(xy_pos.nrows());
            for row in 0..xy_pos.nrows() {
                spots.push((xy_pos[(row, 0)], xy_pos[(row, 1)]));
            }
            series.push(WavelengthSpotSeries {
                wavelength: wvl,
                spots,
            });
        }

        Ok(Self { series })
    }

    /// Returns the total number of ray spots across all spectral bins.
    #[must_use]
    pub fn total_spots(&self) -> usize {
        self.series.iter().map(|s| s.spots.len()).sum()
    }

    /// Returns `true` if no spots are contained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.series.is_empty() || self.total_spots() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{distributions::position::Hexapolar, joule, millimeter, nanometer};
    use uom::si::length::nanometer as Nm;

    #[test]
    fn spot_data_creation_and_bounds() -> OpmResult<()> {
        let rays = Rays::new_uniform_collimated(
            nanometer!(800.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(1.0), 2)?,
        )?;

        let iso = Isometry::identity();
        let data = SpotDiagramData::from_rays(&rays, &iso, nanometer!(1.0))?;

        assert_eq!(data.series.len(), 1);
        assert!(data.total_spots() > 0);
        assert!(!data.is_empty());
        assert_eq!(data.series[0].wavelength.get::<Nm>(), 800.0);
        Ok(())
    }
}
