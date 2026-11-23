//! Strategies for fluence estimation

use crate::utils::default_from_name::DefaultFromName;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::EnumIter;

/// Strategy for fluence estimation
#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, EnumIter, Copy)]
#[non_exhaustive]
pub enum FluenceEstimator {
    /// Calculate Voronoi cells of the hit points and use the cell area for calculation of the fluence.
    #[default]
    Voronoi,
    /// Calculate the fluence at given point using a Kernel Density Estimator
    KDE,
    /// Simply perform binning of the hit points on a given matrix
    Binning,
    /// Using additional "helper rays" for each ray to calculate the evolution of a small area element around the intial ray to calcuklate the fluence
    HelperRays,
}
impl Display for FluenceEstimator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Voronoi => write!(f, "Voronoi"),
            Self::KDE => write!(f, "KDE"),
            Self::Binning => write!(f, "Binning"),
            Self::HelperRays => write!(f, "Helper Rays"),
        }
    }
}
impl DefaultFromName for FluenceEstimator {}
impl FluenceEstimator {
    /// Whether a fluence map of this estimator follows the ray tubes of the sources (see
    /// [`RayMesh`](crate::light::RayMesh)).
    #[must_use]
    pub const fn needs_ray_tubes(self) -> bool {
        matches!(self, Self::Voronoi | Self::HelperRays)
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
    /// The maps of Voronoi and helper rays follow the ray tubes; KDE and binning do not.
    #[test]
    fn only_voronoi_and_helper_rays_follow_ray_tubes() {
        assert!(FluenceEstimator::Voronoi.needs_ray_tubes());
        assert!(FluenceEstimator::HelperRays.needs_ray_tubes());
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
