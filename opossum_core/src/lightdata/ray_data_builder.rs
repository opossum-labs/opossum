//! Builder for the generation of [`LightData`].
//!
//! This module provides a builder for the generation of [`LightData`] to be used in `Source`.
//! This builder allows easier serialization / deserialization in OPM files.
use std::{fmt::Display, path::PathBuf};

use super::LightData;
use crate::{
    degree, energy_distributions::EnergyDistType, error::OpmResult, joule, meter, nanometer,
    position_distributions::PosDistType, rays::Rays, spectral_distribution::SpecDistType,
    utils::default_from_name::DefaultFromName,
};
use serde::{Deserialize, Serialize};
use strum::EnumIter;
use uom::si::{
    f64::{Angle, Energy, Length},
    length::meter,
};

/// Builder for the generation of [`LightData::Geometric`].
#[derive(Clone, Serialize, Deserialize, PartialEq, EnumIter)]
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
    Image(ImageSrc),
}

// impl Validate for RayDataBuilder{
//     fn validate(&self) -> OpmResult<()>{
//         match self{
//             RayDataBuilder::Raw(rays) => rays.validate(),
//             RayDataBuilder::Collimated(collimated_src) => collimated_src.validate(),
//             RayDataBuilder::PointSrc(point_src) => point_src.validate(),
//             RayDataBuilder::Image(image_src) => image_src.validate(),
//         }
//     }
// }

impl From<ImageSrc> for RayDataBuilder {
    fn from(value: ImageSrc) -> Self {
        Self::Image(value)
    }
}

impl From<PointSrc> for RayDataBuilder {
    fn from(value: PointSrc) -> Self {
        Self::PointSrc(value)
    }
}

impl From<CollimatedSrc> for RayDataBuilder {
    fn from(value: CollimatedSrc) -> Self {
        Self::Collimated(value)
    }
}

impl From<Rays> for RayDataBuilder {
    fn from(value: Rays) -> Self {
        Self::Raw(value)
    }
}

impl DefaultFromName for RayDataBuilder {}

/// Represents a collimated source, holding he distributions of the rays for ray tracing,
/// storing distributions related to position, energy, and spectrum.
///
/// # Fields
///
/// * `pos` - Position distribution (`PosDistType`) describing spatial distribution.
/// * `energy` - Energy distribution (`EnergyDistType`) describing energy values of the rays.
/// * `spect` - Spectral distribution (`SpecDistType`) defining wavelength properties.
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollimatedSrc {
    pos: PosDistType,
    energy: EnergyDistType,
    spect: SpecDistType,
}

impl CollimatedSrc {
    /// Creates a new `CollimatedSrc` with specified position, energy, and spectral distributions.
    ///
    /// # Parameters
    ///
    /// * `pos` - Position distribution.
    /// * `energy` - Energy distribution.
    /// * `spect` - Spectral distribution.
    ///
    /// # Returns
    ///
    /// A new instance of `CollimatedSrc`.
    #[must_use]
    pub const fn new(pos: PosDistType, energy: EnergyDistType, spect: SpecDistType) -> Self {
        Self { pos, energy, spect }
    }

    /// Returns a reference to the position distribution.
    #[must_use]
    pub const fn pos_dist(&self) -> &PosDistType {
        &self.pos
    }

    /// Returns a reference to the energy distribution.
    #[must_use]
    pub const fn energy_dist(&self) -> &EnergyDistType {
        &self.energy
    }

    /// Returns a reference to the spectral distribution.
    #[must_use]
    pub const fn spect_dist(&self) -> &SpecDistType {
        &self.spect
    }

    /// Returns a mutable reference to the position distribution.
    pub const fn pos_dist_mut(&mut self) -> &mut PosDistType {
        &mut self.pos
    }

    /// Returns a mutable reference to the energy distribution.
    pub const fn energy_dist_mut(&mut self) -> &mut EnergyDistType {
        &mut self.energy
    }

    /// Returns a mutable reference to the spectral distribution.
    pub const fn spect_dist_mut(&mut self) -> &mut SpecDistType {
        &mut self.spect
    }

    /// Sets the position distribution.
    ///
    /// # Parameters
    ///
    /// * `pos` - New position distribution.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current position distribution.
    pub const fn set_pos_dist(&mut self, pos_dist: PosDistType) {
        self.pos = pos_dist;
    }

    /// Sets the energy distribution.
    ///
    /// # Parameters
    ///
    /// * `energy` - New energy distribution.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current energy distribution.
    pub const fn set_energy_dist(&mut self, energy_dist: EnergyDistType) {
        self.energy = energy_dist;
    }

    /// Sets the spectral distribution.
    ///
    /// # Parameters
    ///
    /// * `spect` - New spectral distribution.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current spectral distribution.
    pub fn set_spect_dist(&mut self, spect_dist: SpecDistType) {
        self.spect = spect_dist;
    }
}

// impl Validate for CollimatedSrc{
//     fn validate(&self) -> OpmResult<()>{
//         self.pos_dist().validate()?;
//         self.energy_dist().validate()?;
//         self.spect_dist().validate()?;
//         Ok(())
//     }
// }

/// Represents a point source for ray tracing,
/// storing various distributions related to position, energy, and spectrum,
/// along with a reference length.
///
/// # Fields
///
/// * `pos` - Position distribution (`PosDistType`) determining how points are spatially distributed.
/// * `energy` - Energy distribution (`EnergyDistType`) describing energy values for the rays.
/// * `spect` - Spectral distribution (`SpecDistType`) defining wavelength properties of the rays.
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
    /// * `pos` - Position distribution.
    /// * `energy` - Energy distribution.
    /// * `spect` - Spectral distribution.
    /// * `reference_length` - Reference length scale.
    ///
    /// # Returns
    ///
    /// A new instance of `PointSrc`.
    #[must_use]
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
    #[must_use]
    pub const fn pos_dist(&self) -> &PosDistType {
        &self.pos_dist
    }

    /// Returns a reference to the energy distribution.
    #[must_use]
    pub const fn energy_dist(&self) -> &EnergyDistType {
        &self.energy_dist
    }

    /// Returns a reference to the spectral distribution.
    #[must_use]
    pub const fn spect_dist(&self) -> &SpecDistType {
        &self.spect_dist
    }

    /// Returns a reference to the reference length.
    #[must_use]
    pub const fn reference_length(&self) -> &Length {
        &self.reference_length
    }

    /// Returns a mutable reference to the position distribution.
    pub const fn pos_dist_mut(&mut self) -> &mut PosDistType {
        &mut self.pos_dist
    }

    /// Returns a mutable reference to the energy distribution.
    pub const fn energy_dist_mut(&mut self) -> &mut EnergyDistType {
        &mut self.energy_dist
    }

    /// Returns a mutable reference to the spectral distribution.
    pub const fn spect_dist_mut(&mut self) -> &mut SpecDistType {
        &mut self.spect_dist
    }

    /// Returns a mutable reference to the reference length.
    pub const fn reference_length_mut(&mut self) -> &mut Length {
        &mut self.reference_length
    }

    /// Sets the position distribution.
    ///
    /// # Parameters
    ///
    /// * `pos` - New position distribution.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current position distribution.
    pub const fn set_pos_dist(&mut self, pos_dist: PosDistType) {
        self.pos_dist = pos_dist;
    }

    /// Sets the energy distribution.
    ///
    /// # Parameters
    ///
    /// * `energy` - New energy distribution.
    ///
    /// # Side Effects
    ///
    /// Overwrites the current energy distribution.
    pub const fn set_energy_dist(&mut self, energy_dist: EnergyDistType) {
        self.energy_dist = energy_dist;
    }

    /// Sets the spectral distribution.
    ///
    /// # Parameters
    ///
    /// * `spect` - New spectral distribution.
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

/// A ray source that emits rays from an image, with a defined cone angle per pixel.
///
/// `ImageSrc` is used to simulate image-based light sources in optical setups.
/// It emits rays from an image plane, where each pixel launches rays within a
/// defined cone angle. This is particularly useful for visualizing image formation,
/// focus planes, or blur depending on the optical system.
///
/// # Fields
/// - `file_path`: Path to the input image file.
/// - `pixel_size`: Size of each pixel on the image plane (usually in millimeters).
/// - `total_energy`: Total energy emitted by the source.
/// - `wave_length`: Wavelength of emitted light.
/// - `cone_angle`: Angular spread of rays emitted from each pixel.
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageSrc {
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
}

impl ImageSrc {
    /// Creates a new [`ImageSrc`] instance with the given configuration.
    ///
    /// # Parameters
    /// - `file_path`: Path to the source image.
    /// - `pixel_size`: Physical size of one image pixel.
    /// - `total_energy`: Total emitted energy.
    /// - `wave_length`: Wavelength of the emitted light.
    /// - `cone_angle`: Cone angle (angular divergence) of rays per pixel.
    ///
    /// # Returns
    /// A new [`ImageSrc`] instance.
    #[must_use]
    pub fn new(
        file_path: PathBuf,
        pixel_size: Length,
        total_energy: Energy,
        wave_length: Length,
        cone_angle: Angle,
    ) -> Self {
        Self {
            file_path,
            pixel_size,
            total_energy,
            wave_length,
            cone_angle,
        }
    }

    /// Returns a reference to the file path of the image source.
    #[must_use]
    pub const fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    /// Sets a new file path for the image source.
    ///
    /// # Parameters
    /// - `f_path`: New path to the image.
    pub fn set_file_path(&mut self, f_path: PathBuf) {
        self.file_path = f_path;
    }

    /// Returns the pixel size in physical units.
    #[must_use]
    pub fn pixel_size(&self) -> Length {
        self.pixel_size
    }

    /// Sets the pixel size.
    ///
    /// # Parameters
    /// - `pixel_size`: New physical size of one pixel.
    pub fn set_pixel_size(&mut self, pixel_size: Length) {
        self.pixel_size = pixel_size;
    }

    /// Returns the total energy of the source.
    #[must_use]
    pub fn energy(&self) -> Energy {
        self.total_energy
    }

    /// Sets the total energy emitted by the source.
    ///
    /// # Parameters
    /// - `energy`: New total energy.
    pub fn set_energy(&mut self, energy: Energy) {
        self.total_energy = energy;
    }

    /// Returns the wavelength of the emitted rays.
    #[must_use]
    pub fn wavelength(&self) -> Length {
        self.wave_length
    }

    /// Sets the wavelength of the emitted rays.
    ///
    /// # Parameters
    /// - `wavelength`: New wavelength.
    pub fn set_wavelength(&mut self, wavelength: Length) {
        self.wave_length = wavelength;
    }

    /// Returns the cone angle of the rays emitted from each pixel.
    #[must_use]
    pub fn cone_angle(&self) -> Angle {
        self.cone_angle
    }

    /// Sets the cone angle for the rays emitted from each pixel.
    ///
    /// # Parameters
    /// - `cone_angle`: New angular spread of rays.
    pub fn set_cone_angle(&mut self, cone_angle: Angle) {
        self.cone_angle = cone_angle;
    }
}

impl Default for ImageSrc {
    /// Returns a default [`ImageSrc`] instance with placeholder values:
    ///
    /// - `file_path`: Empty [`PathBuf`].
    /// - `pixel_size`: 1 mm.
    /// - `total_energy`: 1 joule.
    /// - `wave_length`: 550 nm.
    /// - `cone_angle`: 5 degrees.
    ///
    /// These defaults are useful as initial placeholders for user interfaces
    /// or tests, but they should be replaced with actual data for simulations.
    fn default() -> Self {
        Self {
            file_path: PathBuf::new(),
            pixel_size: nanometer!(5860.),
            total_energy: joule!(0.1),
            wave_length: nanometer!(1054.0),
            cone_angle: degree!(5.0),
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
            Self::Image(image_src) => Ok(LightData::Geometric(Rays::from_image(
                &image_src.file_path,
                image_src.pixel_size,
                image_src.total_energy,
                image_src.wave_length,
                image_src.cone_angle,
            )?)),
        }
    }

    /// Set the position distribution type for the ray source.
    ///
    /// This function sets the [`PosDistType`] on either a collimated or point source variant
    /// of the builder. It has no effect if the builder is in another variant (e.g., `Raw` or `Image`).
    ///
    /// # Parameters
    /// - `pos_dist_type`: The position distribution type to apply.
    ///
    pub const fn set_pos_dist(&mut self, pos_dist_type: PosDistType) {
        match self {
            Self::Collimated(collimated_src) => {
                collimated_src.set_pos_dist(pos_dist_type);
            }
            Self::PointSrc(point_src) => point_src.set_pos_dist(pos_dist_type),
            _ => {}
        }
    }
    /// Set the energy distribution type for the ray source.
    ///
    /// This function sets the [`EnergyDistType`] on either a collimated or point source variant
    /// of the builder. It has no effect if the builder is in another variant.
    ///
    /// # Parameters
    /// - `energy_dist_type`: The energy distribution type to apply.
    ///
    pub const fn set_energy_dist(&mut self, energy_dist_type: EnergyDistType) {
        match self {
            Self::Collimated(collimated_src) => {
                collimated_src.set_energy_dist(energy_dist_type);
            }
            Self::PointSrc(point_src) => point_src.set_energy_dist(energy_dist_type),
            _ => {}
        }
    }
    /// Set the spectral distribution type for the ray source.
    ///
    /// This function sets the [`SpecDistType`] on either a collimated or point source variant
    /// of the builder. It has no effect if the builder is in another variant.
    ///
    /// # Parameters
    /// - `spect_dist_type`: The spectral distribution type to apply.
    ///
    pub fn set_spectral_dist(&mut self, spect_dist_type: SpecDistType) {
        match self {
            Self::Collimated(collimated_src) => {
                collimated_src.set_spect_dist(spect_dist_type);
            }
            Self::PointSrc(point_src) => point_src.set_spect_dist(spect_dist_type),
            _ => {}
        }
    }

    /// Get the position distribution type, if applicable.
    ///
    /// Returns the [`PosDistType`] used in the ray data builder,
    /// if the variant supports it. Available for collimated and point sources.
    ///
    /// Returns `None` if the builder is a variant without position distribution configuration (e.g., `Raw` or `Image`).
    ///
    /// # Returns
    /// - `Some(PosDistType)` if available.
    /// - `None` otherwise.
    #[must_use]
    pub const fn get_position_distribution_type(&self) -> Option<PosDistType> {
        match self {
            Self::Collimated(collimated_src) => Some(*collimated_src.pos_dist()),
            Self::PointSrc(point_src) => Some(*point_src.pos_dist()),
            Self::Raw(_) | Self::Image(_) => None,
        }
    }
    /// Get the energy distribution type, if applicable.
    ///
    /// Returns the [`EnergyDistType`] used in the ray data builder,
    /// if the variant supports it. Available for collimated and point sources.
    ///
    /// Returns `None` if the builder is a variant without energy distribution configuration (e.g., `Raw` or `Image`).
    ///
    /// # Returns
    /// - `Some(EnergyDistType)` if available.
    /// - `None` otherwise.
    #[must_use]
    pub const fn get_energy_distribution_type(&self) -> Option<EnergyDistType> {
        match self {
            Self::Collimated(collimated_src) => Some(*collimated_src.energy_dist()),
            Self::PointSrc(point_src) => Some(*point_src.energy_dist()),
            Self::Raw(_) | Self::Image(_) => None,
        }
    }

    /// Get the spectral distribution type, if applicable.
    ///
    /// Returns the [`SpecDistType`] used in the ray data builder,
    /// if the variant supports it. Available for collimated and point sources.
    ///
    /// Returns `None` if the builder is using a variant without spectral distribution configuration (e.g., `Raw` or `Image`).
    ///
    /// # Returns
    /// - `Some(SpecDistType)` if available.
    /// - `None` otherwise.
    #[must_use]
    pub fn get_spectral_distribution_type(&self) -> Option<SpecDistType> {
        match self {
            Self::Collimated(collimated_src) => Some(collimated_src.spect_dist().clone()),
            Self::PointSrc(point_src) => Some(point_src.spect_dist().clone()),
            Self::Raw(_) | Self::Image(_) => None,
        }
    }
}

impl std::fmt::Debug for RayDataBuilder {
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
            Self::Image(image_src) => {
                write!(
                    f,
                    "Image field({}, {:?}, {:?}, {:?}, {:?}",
                    image_src.file_path.display(),
                    image_src.pixel_size,
                    image_src.total_energy,
                    image_src.wave_length,
                    image_src.cone_angle
                )
            }
        }
    }
}

impl Display for RayDataBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw(r) => write!(f, "Raw({r})"),
            Self::Collimated(_) => {
                write!(f, "Collimated",)
            }
            Self::PointSrc(_) => {
                write!(f, "Point source",)
            }
            Self::Image(_) => {
                write!(f, "Image",)
            }
        }
    }
}
