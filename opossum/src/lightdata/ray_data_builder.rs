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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollimatedSrc {
    /// Position distribution.
    pos_dist: PosDistType,
    /// Energy distribution.
    energy_dist: EnergyDistType,
    /// Wavelength of the rays.
    spect_dist: SpecDistType,
}

impl CollimatedSrc {
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
    pub fn pos_dist(&self) -> &PosDistType {
        &self.pos_dist
    }
    pub fn energy_dist(&self) -> &EnergyDistType {
        &self.energy_dist
    }
    pub fn spect_dist(&self) -> &SpecDistType {
        &self.spect_dist
    }
    pub fn pos_dist_mut(&mut self) -> &mut PosDistType {
        &mut self.pos_dist
    }
    pub fn energy_dist_mut(&mut self) -> &mut EnergyDistType {
        &mut self.energy_dist
    }
    pub fn spect_dist_mut(&mut self) -> &mut SpecDistType {
        &mut self.spect_dist
    }
    pub fn set_pos_dist(&mut self, pos_dist: PosDistType) {
        self.pos_dist = pos_dist;
    }
    pub fn set_energy_dist(&mut self, energy_dist: EnergyDistType) {
        self.energy_dist = energy_dist;
    }
    pub fn set_spect_dist(&mut self, spect_dist: SpecDistType) {
        self.spect_dist = spect_dist;
    }
}

impl Default for CollimatedSrc {
    fn default() -> Self {
        Self {
            pos_dist: PosDistType::default(),
            energy_dist: EnergyDistType::default(),
            spect_dist: SpecDistType::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PointSrc {
    /// Position distribution.
    pos_dist: PosDistType,
    /// Energy distribution.
    energy_dist: EnergyDistType,
    /// Wavelength of the rays.
    spect_dist: SpecDistType,
    /// Length
    reference_length: Length,
}

impl PointSrc {
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
    pub fn pos_dist(&self) -> &PosDistType {
        &self.pos_dist
    }
    pub fn energy_dist(&self) -> &EnergyDistType {
        &self.energy_dist
    }
    pub fn spect_dist(&self) -> &SpecDistType {
        &self.spect_dist
    }
    pub fn reference_length(&self) -> &Length {
        &self.reference_length
    }
    pub fn pos_dist_mut(&mut self) -> &mut PosDistType {
        &mut self.pos_dist
    }
    pub fn energy_dist_mut(&mut self) -> &mut EnergyDistType {
        &mut self.energy_dist
    }
    pub fn spect_dist_mut(&mut self) -> &mut SpecDistType {
        &mut self.spect_dist
    }
    pub fn reference_length_mut(&mut self) -> &mut Length {
        &mut self.reference_length
    }
    pub fn set_pos_dist(&mut self, pos_dist: PosDistType) {
        self.pos_dist = pos_dist;
    }
    pub fn set_energy_dist(&mut self, energy_dist: EnergyDistType) {
        self.energy_dist = energy_dist;
    }
    pub fn set_spect_dist(&mut self, spect_dist: SpecDistType) {
        self.spect_dist = spect_dist;
    }
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

    pub fn set_energy(&mut self, energy: Energy) {
        match self {
            RayDataBuilder::Raw(rays) => todo!(),
            RayDataBuilder::Collimated(collimated_src) => {
                collimated_src.energy_dist_mut().set_energy(energy);
            }
            RayDataBuilder::PointSrc(point_src) => {
                point_src.energy_dist_mut().set_energy(energy);
            }
            RayDataBuilder::Image {
                file_path,
                pixel_size,
                total_energy,
                wave_length,
                cone_angle,
            } => {
                *total_energy = energy;
            }
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
                write!(f, "Image field({file_path:?}, {pixel_size:?}, {total_energy:?}, {wave_length:?}, {cone_angle:?})")
            }
        }
    }
}
