#![warn(missing_docs)]
//! What a user sets for one surface of a component: its coating and its laser induced damage
//! threshold (LIDT).
//!
//! A node keeps one [`SurfaceConfig`] per face of its geometry as a property, named after the face
//! (see [`GeometryKind::face_name`](crate::geometry::GeometryKind::face_name)).
use serde::{Deserialize, Serialize};

use crate::{
    J_per_cm2,
    coatings::CoatingType,
    error::{OpmResult, OpossumError},
    generic_validators::{AllNotNan, AllPositive},
    nodes::fluence_detector::Fluence,
    validated, validated_type,
};

/// A fluence that is neither negative nor NaN, as an LIDT has to be.
pub type ValidatedLidt = validated_type!(Fluence, AllPositive && AllNotNan);

/// The default LIDT: infinite, a surface that cannot be damaged.
fn default_lidt() -> ValidatedLidt {
    validated!(J_per_cm2!(f64::INFINITY), AllPositive && AllNotNan).unwrap()
}
/// Return whether the LIDT is the default, an infinite one.
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_default_lidt(lidt: &ValidatedLidt) -> bool {
    lidt.get().value.is_infinite()
}

/// The coating and the laser induced damage threshold (LIDT) of one surface of a component.
///
/// The default is an ideal antireflective coating and an infinite LIDT. The coating is always
/// written: an empty entry would not read back through the fault-tolerant
/// [`Properties`](crate::properties::Properties) reader.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceConfig {
    #[serde(default)]
    coating: CoatingType,
    #[serde(default = "default_lidt", skip_serializing_if = "is_default_lidt")]
    lidt: ValidatedLidt,
}

impl SurfaceConfig {
    /// Create a new [`SurfaceConfig`].
    ///
    /// # Arguments
    ///
    /// * `coating` - the coating of the surface.
    /// * `lidt` - its laser induced damage threshold; infinite for a surface that cannot be
    ///   damaged.
    ///
    /// # Errors
    ///
    /// This function returns an error if the coating is not valid (see [`CoatingType::is_valid`]),
    /// or the LIDT is negative or NaN.
    pub fn new(coating: CoatingType, lidt: Fluence) -> OpmResult<Self> {
        if !coating.is_valid()? {
            return Err(OpossumError::Properties(format!(
                "the coating {coating:?} is outside its allowed range"
            )));
        }
        Ok(Self {
            coating,
            lidt: validated!(lidt, AllPositive && AllNotNan)?,
        })
    }
    /// Return the coating of the surface.
    #[must_use]
    pub const fn coating(&self) -> &CoatingType {
        &self.coating
    }
    /// Return the laser induced damage threshold of the surface.
    #[must_use]
    pub fn lidt(&self) -> Fluence {
        *self.lidt.get()
    }
}

impl Default for SurfaceConfig {
    fn default() -> Self {
        Self {
            coating: CoatingType::IdealAR,
            lidt: default_lidt(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        coatings::SplittingConfigBuilder,
        nanometer,
        nodes::ideal_filter::{EdgeFilter, EdgeFilterType, SpectralFilterBuilder},
        properties::{Properties, Proptype},
        utils::test_helper::helper::check_logs,
    };

    #[test]
    fn an_invalid_lidt_is_rejected() {
        for lidt in [-0.1, f64::NAN] {
            assert!(SurfaceConfig::new(CoatingType::IdealAR, J_per_cm2!(lidt)).is_err());
        }
        assert!(SurfaceConfig::new(CoatingType::Fresnel, J_per_cm2!(f64::INFINITY)).is_ok());
    }
    #[test]
    fn an_invalid_coating_is_rejected() {
        let lidt = J_per_cm2!(f64::INFINITY);
        for transmission in [-0.01, 1.01] {
            let coating = CoatingType::Separator(transmission.into());
            assert!(SurfaceConfig::new(coating, lidt).is_err());
        }
        assert!(SurfaceConfig::new(CoatingType::Separator(1.0.into()), lidt).is_ok());
    }
    /// A default surface, which leaves out its infinite LIDT, reads back as a property of a node
    /// file, and so does one with both fields set, and one with a separator whose spectrum is
    /// defined by filter parameters.
    #[test]
    fn a_surface_property_reads_back() -> OpmResult<()> {
        let to_error = |e: &dyn std::fmt::Display| OpossumError::Other(e.to_string());
        let short_pass =
            SplittingConfigBuilder::Spectrum(SpectralFilterBuilder::EdgeFilter(EdgeFilter::new(
                EdgeFilterType::ShortPass,
                nanometer!(1000.0),
                0.0..1.0,
                None,
                nanometer!(500.0)..nanometer!(1500.0),
                nanometer!(1.0),
            )?));
        for config in [
            SurfaceConfig::default(),
            SurfaceConfig::new(CoatingType::Fresnel, J_per_cm2!(2.0))?,
            SurfaceConfig::new(short_pass.into(), J_per_cm2!(2.0))?,
        ] {
            let mut props = Properties::default();
            props.create("surface", "a surface", config.clone().into())?;
            let stored = ron::to_string(&props).map_err(|e| to_error(&e))?;
            testing_logger::setup();
            let read: Properties = ron::from_str(&stored).map_err(|e| to_error(&e))?;
            check_logs(log::Level::Warn, vec![]);
            assert_eq!(read.get("surface")?, &Proptype::SurfaceConfig(config));
        }
        Ok(())
    }
}
