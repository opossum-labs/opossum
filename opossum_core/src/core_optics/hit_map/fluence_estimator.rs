//! Strategies for fluence estimation

use crate::utils::default_from_name::DefaultFromName;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::EnumIter;

/// Strategy for fluence estimation
#[derive(
    Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, EnumIter, Copy, JsonSchema,
)]
#[non_exhaustive]
pub enum FluenceEstimator {
    /// Follow the ray tubes the source spans: each corner spreads its energy over the area it
    /// stands for at the source, grown as much as its tubes grew on the way (see the handbook,
    /// "Fluence").
    #[default]
    Voronoi,
    /// Calculate the fluence at given point using a Kernel Density Estimator
    KDE,
    /// Simply perform binning of the hit points on a given matrix
    Binning,
}
impl Display for FluenceEstimator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Voronoi => write!(f, "Voronoi"),
            Self::KDE => write!(f, "KDE"),
            Self::Binning => write!(f, "Binning"),
        }
    }
}
impl DefaultFromName for FluenceEstimator {}
impl FluenceEstimator {
    /// Whether a fluence map of this estimator follows the ray tubes of the sources (see
    /// [`RayMesh`](crate::light::RayMesh)).
    #[must_use]
    pub const fn needs_ray_tubes(self) -> bool {
        matches!(self, Self::Voronoi)
    }
}

#[cfg(test)]
mod test {
    use crate::{core_optics::hit_map::fluence_estimator::FluenceEstimator, properties::Proptype};

    #[test]
    fn fmt() {
        assert_eq!(format!("{}", FluenceEstimator::Voronoi), "Voronoi");
        assert_eq!(format!("{}", FluenceEstimator::KDE), "KDE");
        assert_eq!(format!("{}", FluenceEstimator::Binning), "Binning");
    }
    /// The maps of Voronoi follow the ray tubes; KDE and binning do not.
    #[test]
    fn only_voronoi_follows_ray_tubes() {
        assert!(FluenceEstimator::Voronoi.needs_ray_tubes());
        assert!(!FluenceEstimator::KDE.needs_ray_tubes());
        assert!(!FluenceEstimator::Binning.needs_ray_tubes());
    }
    #[test]
    fn from() {
        assert!(matches!(
            FluenceEstimator::Voronoi.into(),
            Proptype::FluenceEstimator(_)
        ));
    }
}
