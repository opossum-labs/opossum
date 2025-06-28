//! Builder for the generation of [`LightData`].
//!
//! This module provides a builder for the generation of [`LightData`] to be used in `Source`.
//! This builder allows easier serialization / deserialization in OPM files.
use std::{fmt::Display, path::PathBuf};

use super::LightData;
use crate::{
    energy_distributions::EnergyDistType, error::OpmResult, meter,
    position_distributions::PosDistType, rays::Rays, spectral_distribution::SpecDistType,
};
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::{Angle, Energy, Length},
    length::meter,
};

/// Builder for the generation of [`LightData::Geometric`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RayDataBuilder {
    /// Raw [`Rays`] data.
    Raw(Rays),
    /// Collimated [`Rays`] data with a given [`PosDistType`], [`EnergyDistType`], and [`SpecDistType`].
    Collimated(CollimatedSrc),
    /// Point source [`Rays`] data with a given [`PosDistType`], [`EnergyDistType`], and [`SpecDistType`].
    /// All rays start on the optical axis and are emitted within a cone. The cone is defined by the
    /// position distribution **after the rays have propagated the given reference length**.
    PointSrc(PointSrc),
    /// A bundle of rays emitted from a 2D black & white image specified by its file path, the actual (x/y) dimenstions of the image as well as the
    /// total energy.
    Image {
        /// path to the image file
        file_path: PathBuf,
        /// x & y dimensions of the image
        pixel_size: Length,
        /// total energy
        total_energy: Energy,
        /// wavelength
        wave_length: Length,
        /// cone angle of each point src per pixel
        cone_angle: Angle,
    },
}
/// Represents a collimated source for ray tracing,
/// storing distributions related to position, energy, and spectrum.
///
/// # Fields
///
/// * `pos_dist` - Position distribution (`PosDistType`) describing spatial distribution.
/// * `energy_dist` - Energy distribution (`EnergyDistType`) describing energy values of the rays.
/// * `spect_dist` - Spectral distribution (`SpecDistType`) defining wavelength properties.
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollimatedSrc {
    pos_dist: PosDistType,
    energy_dist: EnergyDistType,
    spect_dist: SpecDistType,
}

impl CollimatedSrc {
    /// Creates a new `CollimatedSrc` with specified position, energy, and spectral distributions.
    ///
    /// # Parameters
    ///
    /// * `pos_dist` - Position distribution.
    /// * `energy_dist` - Energy distribution.
    /// * `spect_dist` - Spectral distribution.
    ///
    /// # Returns
    ///
    /// A new instance of `CollimatedSrc`.
    pub fn new(
        pos_dist: PosDistType,
        energy_dist: EnergyDistType,
        spect_dist: SpecDistType,
    ) -> Self {
        Self {
            pos_dist,
            energy_dist,
            spect_dist,
        }
    }

    /// Returns a reference to the position distribution.
    pub fn pos_dist(&self) -> &PosDistType {
        &self.pos_dist
    }

    /// Returns a reference to the energy distribution.
    pub fn energy_dist(&self) -> &EnergyDistType {
        &self.energy_dist
    }

    /// Returns a reference to the spectral distribution.
    pub fn spect_dist(&self) -> &SpecDistType {
        &self.spect_dist
    }

    /// Returns a mutable reference to the position distribution.
    pub fn pos_dist_mut(&mut self) -> &mut PosDistType {
        &mut self.pos_dist
    }

    /// Returns a mutable reference to the energy distribution.
    pub fn energy_dist_mut(&mut self) -> &mut EnergyDistType {
        &mut self.energy_dist
    }

    /// Returns a mutable reference to the spectral distribution.
    pub fn spect_dist_mut(&mut self) -> &mut SpecDistType {
        &mut self.spect_dist
    }

    /// Sets the position distribution.
    ///
    /// # Parameters
    ///
    /// * `pos_dist` - New position distribution.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current position distribution.
    pub fn set_pos_dist(&mut self, pos_dist: PosDistType) {
        self.pos_dist = pos_dist;
    }

    /// Sets the energy distribution.
    ///
    /// # Parameters
    ///
    /// * `energy_dist` - New energy distribution.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current energy distribution.
    pub fn set_energy_dist(&mut self, energy_dist: EnergyDistType) {
        self.energy_dist = energy_dist;
    }

    /// Sets the spectral distribution.
    ///
    /// # Parameters
    ///
    /// * `spect_dist` - New spectral distribution.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current spectral distribution.
    pub fn set_spect_dist(&mut self, spect_dist: SpecDistType) {
        self.spect_dist = spect_dist;
    }
}

/// Represents a point source for ray tracing,
/// storing various distributions related to position, energy, and spectrum,
/// along with a reference length.
///
/// # Fields
///
/// * `pos_dist` - Position distribution (`PosDistType`) determining how points are spatially distributed.
/// * `energy_dist` - Energy distribution (`EnergyDistType`) describing energy values for the rays.
/// * `spect_dist` - Spectral distribution (`SpecDistType`) defining wavelength properties of the rays.
/// * `reference_length` - A length scale used as a reference in calculations (`Length`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PointSrc {
    pos_dist: PosDistType,
    energy_dist: EnergyDistType,
    spect_dist: SpecDistType,
    reference_length: Length,
}

impl PointSrc {
    /// Creates a new `PointSrc` with specified distributions and reference length.
    ///
    /// # Parameters
    ///
    /// * `pos_dist` - Position distribution.
    /// * `energy_dist` - Energy distribution.
    /// * `spect_dist` - Spectral distribution.
    /// * `reference_length` - Reference length scale.
    ///
    /// # Returns
    ///
    /// A new instance of `PointSrc`.
    pub fn new(
        pos_dist: PosDistType,
        energy_dist: EnergyDistType,
        spect_dist: SpecDistType,
        reference_length: Length,
    ) -> Self {
        Self {
            pos_dist,
            energy_dist,
            spect_dist,
            reference_length,
        }
    }

    /// Returns a reference to the position distribution.
    pub fn pos_dist(&self) -> &PosDistType {
        &self.pos_dist
    }

    /// Returns a reference to the energy distribution.
    pub fn energy_dist(&self) -> &EnergyDistType {
        &self.energy_dist
    }

    /// Returns a reference to the spectral distribution.
    pub fn spect_dist(&self) -> &SpecDistType {
        &self.spect_dist
    }

    /// Returns a reference to the reference length.
    pub fn reference_length(&self) -> &Length {
        &self.reference_length
    }

    /// Returns a mutable reference to the position distribution.
    pub fn pos_dist_mut(&mut self) -> &mut PosDistType {
        &mut self.pos_dist
    }

    /// Returns a mutable reference to the energy distribution.
    pub fn energy_dist_mut(&mut self) -> &mut EnergyDistType {
        &mut self.energy_dist
    }

    /// Returns a mutable reference to the spectral distribution.
    pub fn spect_dist_mut(&mut self) -> &mut SpecDistType {
        &mut self.spect_dist
    }

    /// Returns a mutable reference to the reference length.
    pub fn reference_length_mut(&mut self) -> &mut Length {
        &mut self.reference_length
    }

    /// Sets the position distribution.
    ///
    /// # Parameters
    ///
    /// * `pos_dist` - New position distribution.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current position distribution.
    pub fn set_pos_dist(&mut self, pos_dist: PosDistType) {
        self.pos_dist = pos_dist;
    }

    /// Sets the energy distribution.
    ///
    /// # Parameters
    ///
    /// * `energy_dist` - New energy distribution.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current energy distribution.
    pub fn set_energy_dist(&mut self, energy_dist: EnergyDistType) {
        self.energy_dist = energy_dist;
    }

    /// Sets the spectral distribution.
    ///
    /// # Parameters
    ///
    /// * `spect_dist` - New spectral distribution.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current spectral distribution.
    pub fn set_spect_dist(&mut self, spect_dist: SpecDistType) {
        self.spect_dist = spect_dist;
    }

    /// Sets the reference length.
    ///
    /// # Parameters
    ///
    /// * `ref_length` - New reference length.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current reference length.
    pub fn set_reference_length(&mut self, ref_length: Length) {
        self.reference_length = ref_length;
    }
}

impl Default for PointSrc {
    fn default() -> Self {
        Self {
            pos_dist: PosDistType::default(),
            energy_dist: EnergyDistType::default(),
            spect_dist: SpecDistType::default(),
            reference_length: meter!(1.),
        }
    }
}

impl Default for RayDataBuilder {
    fn default() -> Self {
        Self::Collimated(CollimatedSrc::default())
    }
}
impl RayDataBuilder {
    /// Create [`LightData::Geometric`] from the builder definition.
    ///
    /// # Errors
    /// This function will return an error if the concrete implementation of the builder fails.
    pub fn build(self) -> OpmResult<LightData> {
        match self {
            Self::Raw(rays) => Ok(LightData::Geometric(rays)),
            Self::Collimated(collimated_src) => {
                let rays = Rays::new_collimated_with_spectrum(
                    collimated_src.spect_dist().generate(),
                    collimated_src.energy_dist().generate(),
                    collimated_src.pos_dist().generate(),
                )?;
                Ok(LightData::Geometric(rays))
            }
            Self::PointSrc(point_src) => {
                let rays = Rays::new_point_src_with_spectrum(
                    point_src.spect_dist().generate(),
                    point_src.energy_dist().generate(),
                    point_src.pos_dist().generate(),
                    *point_src.reference_length(),
                )?;
                Ok(LightData::Geometric(rays))
            }
            Self::Image {
                file_path,
                pixel_size,
                total_energy,
                wave_length,
                cone_angle,
            } => Ok(LightData::Geometric(Rays::from_image(
                &file_path,
                pixel_size,
                total_energy,
                wave_length,
                cone_angle,
            )?)),
        }
    }
}

impl Display for RayDataBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw(r) => write!(f, "Raw({r})"),
            Self::Collimated(collimated_src) => {
                write!(
                    f,
                    "Collimated({:?}, {:?}, {:?})",
                    collimated_src.pos_dist(),
                    collimated_src.energy_dist(),
                    collimated_src.spect_dist()
                )
            }
            Self::PointSrc(point_src) => {
                write!(
                    f,
                    "PointSrc({:?}, {:?}, {:?}, {}m)",
                    point_src.pos_dist(),
                    point_src.energy_dist(),
                    point_src.spect_dist(),
                    point_src.reference_length().get::<meter>()
                )
            }
            Self::Image {
                file_path,
                pixel_size,
                total_energy,
                wave_length,
                cone_angle,
            } => {
                write!(
                    f,
                    "Image field({}, {pixel_size:?}, {total_energy:?}, {wave_length:?}, {cone_angle:?}",
                    file_path.display()
                )
            }
        }
    }
}
