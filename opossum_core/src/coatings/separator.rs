#![warn(missing_docs)]
use super::{Coating, CoatingType};
use crate::{
    core_optics::{hit_map::rays_hit_map::HitPoint, surface_effect::SurfaceEffect},
    error::{OpmResult, OpossumError},
    geometry::SurfaceHit,
    light::{Ray, spectrum::Spectrum},
    nodes::ideal_filter::SpectralFilterBuilder,
    utils::default_from_name::DefaultFromName,
};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::{EnumIter, IntoEnumIterator};
use uom::si::{
    f64::{Length, Ratio},
    ratio::ratio,
};

/// The definition of a [`SplittingConfig`], as a [`CoatingType::Separator`] stores it.
///
/// A spectrum keeps its filter parameters or its file; it is built once per interaction with a
/// surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter)]
pub enum SplittingConfigBuilder {
    /// a fixed (wavelength-independant) transmission value. Must be between 0.0 and 1.0
    FixedRatio(f64),
    /// splitting based on given transmission spectrum.
    Spectrum(SpectralFilterBuilder),
}

impl SplittingConfigBuilder {
    /// Constructs a [`SplittingConfig`] object from the builder.
    /// # Errors
    /// Returns an error if the creation of a spectrum fails.
    pub fn build(&self) -> OpmResult<SplittingConfig> {
        match self {
            Self::FixedRatio(c) => Ok(SplittingConfig::Ratio(*c)),
            Self::Spectrum(spectral_filter_builder) => {
                Ok(SplittingConfig::Spectrum(spectral_filter_builder.build()?))
            }
        }
    }
    /// Checks whether the [`SplittingConfig`] built from this definition transmits within
    /// `[0.0, 1.0]`.
    ///
    /// # Errors
    ///
    /// This function returns an error if the transmission spectrum cannot be read from its file.
    pub fn is_valid(&self) -> OpmResult<bool> {
        match self {
            Self::FixedRatio(transmission) => Ok((0.0..=1.0).contains(transmission)),
            Self::Spectrum(spectrum) => spectrum.values_are_in_range(0.0, 1.0),
        }
    }
}

impl Default for SplittingConfigBuilder {
    /// A 50:50 split.
    fn default() -> Self {
        Self::FixedRatio(0.5)
    }
}

impl From<SplittingConfigBuilder> for CoatingType {
    fn from(config: SplittingConfigBuilder) -> Self {
        Self::Separator(config)
    }
}

impl From<SpectralFilterBuilder> for SplittingConfigBuilder {
    fn from(val: SpectralFilterBuilder) -> Self {
        Self::Spectrum(val)
    }
}
impl From<f64> for SplittingConfigBuilder {
    fn from(val: f64) -> Self {
        Self::FixedRatio(val)
    }
}

impl Display for SplittingConfigBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FixedRatio(_) => write!(f, "Fixed Ratio"),
            Self::Spectrum(_) => write!(f, "Spectral filter"),
        }
    }
}

impl DefaultFromName for SplittingConfigBuilder {
    fn default_from_name(name: &str) -> Option<Self> {
        for ftb in Self::iter() {
            if name == format!("{ftb}") {
                match ftb {
                    Self::FixedRatio(_) => {
                        return Some(Self::default());
                    }
                    Self::Spectrum(_) => return Some(ftb),
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter)]
/// Configuration for splitting a [`Ray`](crate::light::Ray) into multiple parts.
///
/// This enum defines how a ray is split, either by a fixed ratio or by a wavelength-dependent spectrum.
pub enum SplittingConfig {
    /// Ideal beam splitter with a fixed splitting ratio.
    ///
    /// The `f64` value must be in the range (0.0..=1.0), where 1.0 means all energy remains in the initial beam,
    /// and 0.0 means all energy is transferred to the split beam.
    Ratio(f64),
    /// Beam splitter with a wavelength-dependent transmission spectrum.
    ///
    /// The [`Spectrum`] must contain values in the range (0.0..=1.0).
    Spectrum(Spectrum),
}
impl SplittingConfig {
    /// Checks the validity of the [`SplittingConfig`].
    ///
    /// Returns `true` if all values in the spectrum or the ratio are within the range (0.0..=1.0).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Ratio(r) => (0.0..=1.0).contains(r),
            Self::Spectrum(s) => s.is_transmission_spectrum(),
        }
    }
    /// Returns the transmission of this [`SplittingConfig`] at a given wavelength.
    ///
    /// The transmission is the part of the energy that remains in the initial beam. For
    /// [`SplittingConfig::Ratio`] it is the ratio itself, for [`SplittingConfig::Spectrum`] it is the
    /// spectrum value at the given wavelength.
    ///
    /// # Arguments
    ///
    /// * `wavelength` - the wavelength of the light to be split.
    ///
    /// # Returns
    ///
    /// The transmission in the range `(0.0..=1.0)`.
    ///
    /// # Errors
    ///
    /// This function returns an error if the wavelength is outside the given spectrum or the
    /// transmission is outside the interval `[0.0..1.0]`.
    pub fn transmission(&self, wavelength: Length) -> OpmResult<f64> {
        let transmission = match self {
            Self::Ratio(r) => *r,
            Self::Spectrum(spectrum) => spectrum.get_value(&wavelength).ok_or_else(|| {
                OpossumError::Spectrum(
                    "ray splitting failed. wavelength outside given spectrum".into(),
                )
            })?,
        };
        if !(0.0..=1.0).contains(&transmission) {
            return Err(OpossumError::Other(
                "splitting_ratio must be within [0.0;1.0]".into(),
            ));
        }
        Ok(transmission)
    }
}

/// A splitting surface reflects everything that it does not transmit: its reflectivity is
/// `1 - transmission` at the wavelength of the ray, whatever the angle of incidence or the medium.
impl Coating for SplittingConfig {
    fn calc_reflectivity(
        &self,
        incoming_ray: &Ray,
        _surface_normal: Vector3<f64>,
        _n2: f64,
    ) -> OpmResult<Ratio> {
        Ok(Ratio::new::<ratio>(
            1.0 - self.transmission(incoming_ray.wavelength())?,
        ))
    }
}

/// A splitting surface splits a ray by this [`SplittingConfig`] as its coating; the coating of the
/// surface itself is not used. The transmitted part keeps its direction, the reflected part is
/// mirrored about the surface normal.
impl SurfaceEffect for SplittingConfig {
    fn act_at(
        &self,
        ray: &mut Ray,
        hit: &SurfaceHit,
        _coating: &dyn Coating,
    ) -> OpmResult<(Option<Ray>, Option<HitPoint>)> {
        ray.refract_at(hit, self, None)
    }
}

impl Display for SplittingConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ratio(_) => write!(f, "Fixed Ratio"),
            Self::Spectrum(_) => write!(f, "Split by Spectrum"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        joule, nanometer,
        nodes::ideal_filter::{EdgeFilter, EdgeFilterType},
    };
    use approx::assert_abs_diff_eq;

    fn short_pass_config() -> OpmResult<SplittingConfig> {
        let spectrum: Spectrum = EdgeFilter::new(
            EdgeFilterType::ShortPass,
            nanometer!(1000.0),
            0.0..1.0,
            None,
            nanometer!(500.0)..nanometer!(1500.0),
            nanometer!(1.0),
        )?
        .into();
        Ok(SplittingConfig::Spectrum(spectrum))
    }
    #[test]
    fn splitting_config_transmission() -> OpmResult<()> {
        assert_abs_diff_eq!(
            SplittingConfig::Ratio(0.6).transmission(nanometer!(1000.0))?,
            0.6
        );
        assert!(
            SplittingConfig::Ratio(1.1)
                .transmission(nanometer!(1000.0))
                .is_err()
        );
        assert!(
            SplittingConfig::Ratio(-0.1)
                .transmission(nanometer!(1000.0))
                .is_err()
        );
        let config = short_pass_config()?;
        assert_abs_diff_eq!(config.transmission(nanometer!(999.0))?, 1.0);
        assert_abs_diff_eq!(config.transmission(nanometer!(1001.0))?, 0.0);
        assert!(config.transmission(nanometer!(1501.0)).is_err());
        Ok(())
    }
    #[test]
    fn splitting_config_reflectivity() -> OpmResult<()> {
        let reflectivity = |config: &SplittingConfig, wavelength| -> OpmResult<f64> {
            let ray = Ray::origin_along_z(wavelength, joule!(1.0))?;
            Ok(config
                .calc_reflectivity(&ray, Vector3::z(), 1.5)?
                .get::<ratio>())
        };
        assert_abs_diff_eq!(
            reflectivity(&SplittingConfig::Ratio(0.6), nanometer!(1000.0))?,
            0.4
        );
        assert!(reflectivity(&short_pass_config()?, nanometer!(1501.0)).is_err());
        Ok(())
    }
}
