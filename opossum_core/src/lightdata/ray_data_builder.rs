//! Builder for the generation of [`LightData`].
//!
//! This module provides a builder for the generation of [`LightData`] to be used in `Source`.
//! This builder allows easier serialization / deserialization in OPM files.
use std::{fmt::Display, path::PathBuf};

use super::LightData;
use crate::{
    degree,
    energy_distributions::EnergyDistType,
    error::OpmResult,
    generic_validators::{AllFinite, AllInRange, AllNormal, AllPositive, PathValid},
    joule, meter, nanometer,
    position_distributions::PosDistType,
    rays::Rays,
    spectral_distribution::SpecDistType,
    utils::default_from_name::DefaultFromName,
    validated, validated_type,
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
    reference_length: validated_type!(Length, AllPositive && AllNormal),
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
    ///
    /// # Errors
    /// Returns an error if the validation of the reference length fails
    pub fn new(
        pos_dist: PosDistType,
        energy_dist: EnergyDistType,
        spect_dist: SpecDistType,
        reference_length: Length,
    ) -> OpmResult<Self> {
        Ok(Self {
            pos_dist,
            energy_dist,
            spect_dist,
            reference_length: validated!(reference_length, AllPositive && AllNormal)?,
        })
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
        self.reference_length.get()
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
    pub const fn reference_length_mut(
        &mut self,
    ) -> &mut validated_type!(Length, AllPositive && AllNormal) {
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
    ///
    /// # Errors
    /// Returns an error ofthe validation of the reference length fails
    pub fn set_reference_length(&mut self, ref_length: Length) -> OpmResult<()> {
        self.reference_length.set(ref_length)?;
        Ok(())
    }
}

impl Default for PointSrc {
    fn default() -> Self {
        Self {
            pos_dist: PosDistType::default(),
            energy_dist: EnergyDistType::default(),
            spect_dist: SpecDistType::default(),
            reference_length: validated!(meter!(1.), AllPositive && AllNormal).unwrap(),
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
    file_path: validated_type!(PathBuf, PathValid),
    /// x & y dimensions of the image
    pixel_size: validated_type!(Length, AllPositive && AllNormal),
    /// total energy
    total_energy: validated_type!(Energy, AllPositive && AllNormal),
    /// wavelength
    wave_length: validated_type!(Length, AllPositive && AllNormal),
    /// cone angle of each point src per pixel
    cone_angle: validated_type!(Angle, AllFinite && AllInRange::<Angle>),
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
    ///
    /// # Errors
    /// Returns in error if any of the input arguments are invalid
    pub fn new(
        file_path: PathBuf,
        pixel_size: Length,
        total_energy: Energy,
        wave_length: Length,
        cone_angle: Angle,
    ) -> OpmResult<Self> {
        let mut img_src = Self::default();
        img_src.set_file_path(file_path)?;
        img_src.set_pixel_size(pixel_size)?;
        img_src.set_energy(total_energy)?;
        img_src.set_wavelength(wave_length)?;
        img_src.set_cone_angle(cone_angle)?;
        Ok(img_src)
    }

    /// Returns a reference to the file path of the image source.
    #[must_use]
    pub const fn file_path(&self) -> &PathBuf {
        self.file_path.get()
    }

    /// Sets a new file path for the image source.
    ///
    /// # Parameters
    /// - `f_path`: New path to the image.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_file_path(&mut self, f_path: PathBuf) -> OpmResult<()> {
        self.file_path.set(f_path)?;
        Ok(())
    }

    /// Returns the pixel size in physical units.
    #[must_use]
    pub fn pixel_size(&self) -> Length {
        *self.pixel_size.get()
    }

    /// Sets the pixel size.
    ///
    /// # Parameters
    /// - `pixel_size`: New physical size of one pixel.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_pixel_size(&mut self, pixel_size: Length) -> OpmResult<()> {
        self.pixel_size.set(pixel_size)?;
        Ok(())
    }

    /// Returns the total energy of the source.
    #[must_use]
    pub fn energy(&self) -> Energy {
        *self.total_energy.get()
    }

    /// Sets the total energy emitted by the source.
    ///
    /// # Parameters
    /// - `energy`: New total energy.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_energy(&mut self, energy: Energy) -> OpmResult<()> {
        self.total_energy.set(energy)?;
        Ok(())
    }

    /// Returns the wavelength of the emitted rays.
    #[must_use]
    pub fn wavelength(&self) -> Length {
        *self.wave_length.get()
    }

    /// Sets the wavelength of the emitted rays.
    ///
    /// # Parameters
    /// - `wavelength`: New wavelength.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_wavelength(&mut self, wavelength: Length) -> OpmResult<()> {
        self.wave_length.set(wavelength)?;
        Ok(())
    }

    /// Returns the cone angle of the rays emitted from each pixel.
    #[must_use]
    pub fn cone_angle(&self) -> Angle {
        *self.cone_angle.get()
    }

    /// Sets the cone angle for the rays emitted from each pixel.
    ///
    /// # Parameters
    /// - `cone_angle`: New angular spread of rays.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_cone_angle(&mut self, cone_angle: Angle) -> OpmResult<()> {
        self.cone_angle.set(cone_angle)?;
        Ok(())
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
            file_path: validated!(
                PathBuf::new(),
                PathValid::new(Some(vec!["jpg", "bmp", "png"]))
            )
            .unwrap(),
            pixel_size: validated!(nanometer!(5860.), AllPositive && AllNormal).unwrap(),
            total_energy: validated!(joule!(0.1), AllPositive && AllNormal).unwrap(),
            wave_length: validated!(nanometer!(1054.0), AllPositive && AllNormal).unwrap(),
            cone_angle: validated!(
                degree!(5.0),
                AllFinite && (AllInRange::new(degree!(0.0), degree!(180.0), false).unwrap())
            )
            .unwrap(),
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
                image_src.file_path.get(),
                *image_src.pixel_size.get(),
                *image_src.total_energy.get(),
                *image_src.wave_length.get(),
                *image_src.cone_angle.get(),
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
                    image_src.file_path.get().display(),
                    image_src.pixel_size.get(),
                    image_src.total_energy.get(),
                    image_src.wave_length.get(),
                    image_src.cone_angle.get()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::OpossumError,
        generic_validators::{AllNormal, AllPositive, Validate},
    };

    fn get_length(src: &PointSrc) -> Length {
        *src.reference_length()
    }

    #[test]
    fn default_reference_length_is_valid_and_one_meter() {
        let src = PointSrc::default();
        let len = get_length(&src);

        assert_eq!(len, meter!(1.0));

        assert!(
            AllPositive::validate(&AllPositive, &len).is_ok(),
            "Default must be positive"
        );
        assert!(
            AllNormal::validate(&AllNormal, &len).is_ok(),
            "Default must be finite & normal"
        );

        let validated_value = validated!(len, AllPositive && AllNormal);
        assert!(
            validated_value.is_ok(),
            "Default length must satisfy AllPositive && AllNormal"
        );
    }

    #[test]
    fn set_reference_length_to_valid_positive_value_succeeds() {
        let mut src = PointSrc::default();

        let res = src.set_reference_length(meter!(2.5));

        assert!(
            res.is_ok(),
            "Setting a positive, finite length should succeed"
        );
        assert_eq!(get_length(&src), meter!(2.5));
    }

    #[test]
    fn set_reference_length_to_zero_fails_if_zero_is_not_allowed() {
        let mut src = PointSrc::default();

        let res = src.set_reference_length(meter!(0.0));
        assert!(
            res.is_err(),
            "Zero length should be rejected by AllNormal validator"
        );
    }

    #[test]
    fn set_reference_length_to_negative_value_fails() {
        let mut src = PointSrc::default();

        let res = src.set_reference_length(meter!(-1.0));
        assert!(res.is_err(), "Negative length should be rejected");

        if let Err(OpossumError::Other(msg)) = res {
            assert!(
                msg.contains("positive") || msg.contains("negative"),
                "Error message should mention positivity: {}",
                msg
            );
        }
    }

    #[test]
    fn set_reference_length_to_nan_or_infinite_fails() {
        let mut src = PointSrc::default();

        // NaN
        let res_nan = src.set_reference_length(meter!(f64::NAN));
        assert!(res_nan.is_err(), "NaN should be rejected by AllNormal");

        // +Inf
        let res_inf = src.set_reference_length(meter!(f64::INFINITY));
        assert!(res_inf.is_err(), "Infinity should be rejected by AllNormal");

        // -Inf
        let res_neg_inf = src.set_reference_length(meter!(f64::NEG_INFINITY));
        assert!(res_neg_inf.is_err(), "Negative infinity should be rejected");
    }

    #[test]
    fn validator_combination_is_stable() {
        let value = meter!(1.0);
        let v = validated!(value, AllPositive && AllNormal);
        assert!(
            v.is_ok(),
            "Validator combination for reference_length should remain AllPositive && AllNormal"
        );
    }
}
