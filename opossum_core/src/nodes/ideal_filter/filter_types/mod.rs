mod band;
mod edge;
mod math;
mod spectral_builder;

pub use band::{BandFilter, BandFilterType};
pub use edge::{EdgeFilter, EdgeFilterType};
use opm_macros_lib::EnsureValidated;
pub use spectral_builder::SpectralFilterBuilder;
use uom::si::f64::Ratio;

use std::fmt::Display;

use crate::{
    coatings::Coating,
    core_optics::{
        hit_map::rays_hit_map::HitPoint,
        surface_effect::{Refraction, SurfaceEffect},
    },
    error::{OpmResult, OpossumError},
    generic_validators::{AllFinite, StaticBounds, StaticInRange},
    geometry::SurfaceHit,
    light::{Ray, Spectrum},
    percent,
    utils::default_from_name::DefaultFromName,
    validated, validated_type,
};
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};

#[derive(Copy, Clone, PartialEq, Debug, Eq)]
struct TransmissionBounds;

impl StaticBounds<Ratio> for TransmissionBounds {
    fn min() -> Ratio {
        percent!(0.0)
    }
    fn max() -> Ratio {
        percent!(100.0)
    }
    fn inclusive() -> bool {
        true
    }
}

type ValidatedTransmission = validated_type!(
    Ratio,
    AllFinite && StaticInRange::<Ratio, TransmissionBounds>
);
impl Default for ValidatedTransmission {
    fn default() -> Self {
        validated!(
            percent!(100.0),
            AllFinite && StaticInRange::<Ratio, TransmissionBounds>::default()
        )
        .unwrap()
    }
}

/// Config data for a constant filter type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, EnsureValidated)]
pub struct FilterConst {
    /// the constant transmission value of the filter. Must be between 0.0 and 1.0
    transmission: ValidatedTransmission,
}
impl FilterConst {
    /// Creates a new [`FilterConst`] with the given transmission value.
    ///
    /// # Errors
    ///
    /// Returns an error if the transmission value is not between 0.0 and 1.0 or if it is not a finite number.
    pub fn new(transmission: Ratio) -> OpmResult<Self> {
        let mut new_transmission = ValidatedTransmission::default();
        new_transmission.set(transmission)?;
        Ok(Self {
            transmission: new_transmission,
        })
    }
    /// Returns the transmission value of the filter.
    #[must_use]
    pub const fn transmission(&self) -> &Ratio {
        self.transmission.get()
    }
}
/// Config data builder for an [`IdealFilter`](crate::nodes::IdealFilter).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter)]
pub enum FilterTypeBuilder {
    /// a fixed (wavelength-independant) transmission value. Must be between 0.0 and 1.0
    Constant(FilterConst),
    /// filter based on given transmission spectrum.
    Spectrum(SpectralFilterBuilder),
}
impl Default for FilterTypeBuilder {
    fn default() -> Self {
        Self::Constant(FilterConst::default())
    }
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
            Self::Constant(c) => Ok(FilterType::Constant(c.clone())),
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
                        return Some(Self::Constant(FilterConst::new(percent!(100.0)).unwrap()));
                    }
                    Self::Spectrum(_) => return Some(ftb),
                }
            }
        }
        None
    }
}

/// Config data for an [`IdealFilter`](crate::nodes::IdealFilter).
#[derive(Debug, Clone, PartialEq)]
pub enum FilterType {
    /// a fixed (wavelength-independant) transmission value. Must be between 0.0 and 1.0
    Constant(FilterConst),
    /// filter based on given transmission spectrum.
    Spectrum(Spectrum),
}
/// An ideal filter is a passive surface: its coating splits off a reflected part, and the filter
/// attenuates the transmitted part by its transmission (see [`Ray::filter_energy`]).
impl SurfaceEffect for FilterType {
    fn act_at(
        &self,
        ray: &mut Ray,
        hit: &SurfaceHit,
        coating: &dyn Coating,
    ) -> OpmResult<(Option<Ray>, Option<HitPoint>)> {
        let split = Refraction(None).act_at(ray, hit, coating)?;
        ray.filter_energy(self)?;
        Ok(split)
    }
}
impl TryFrom<f64> for FilterTypeBuilder {
    type Error = OpossumError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Ok(Self::Constant(FilterConst::new(value.into())?))
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::propagation_strategy::MissedSurfaceStrategy,
        apertures::{CircleShape, Outline},
        geometry::{Geometry, SurfaceGeometry},
        joule,
        light::{Rays, rays::ReflectedPart},
        millimeter, nanometer,
    };
    /// A ray that hits a filter keeps its direction and wavelength and is attenuated as a single ray
    /// would be (see [`Ray::filter_energy`]). A ray that misses it keeps its energy.
    #[test]
    fn a_filter_attenuates_the_rays_that_hit_it() -> OpmResult<()> {
        let outline = Outline::new(CircleShape::new(millimeter!(5.0))?, None)?;
        let mut plane = Geometry::plane(Some(outline));
        let ray_at =
            |y| Ray::new_collimated(millimeter!(0.0, y, -10.0), nanometer!(1054.0), joule!(1.0));
        let filter = FilterType::Constant(FilterConst::new(percent!(30.0))?);
        let mut rays = Rays::from(vec![ray_at(1.0)?, ray_at(10.0)?]);
        plane.interact_with_rays(
            SurfaceGeometry::FACE,
            &mut rays,
            &filter,
            ReflectedPart::Ghost,
            MissedSurfaceStrategy::Ignore,
        )?;
        let mut alone = ray_at(1.0)?;
        alone.filter_energy(&filter)?;
        let (hit, missed) = (rays.iter().next(), rays.iter().nth(1));
        let (hit, missed) = (
            hit.expect("the ray hitting"),
            missed.expect("the ray missing"),
        );
        assert_eq!(hit.position(), millimeter!(0.0, 1.0, 0.0));
        assert_eq!(hit.direction(), alone.direction());
        assert_eq!(hit.wavelength(), alone.wavelength());
        assert_eq!(hit.energy(), alone.energy());
        assert_eq!(missed.energy(), joule!(1.0));
        Ok(())
    }
    #[test]
    fn new() {
        assert!(FilterConst::new(percent!(0.0)).is_ok());
        assert!(FilterConst::new(percent!(100.0)).is_ok());
        assert!(FilterConst::new(percent!(-0.1)).is_err());
        assert!(FilterConst::new(percent!(100.1)).is_err());
    }
}
