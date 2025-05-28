//! Strategies for fluence estimation

use crate::properties::Proptype;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Strategy for fluence estimation
#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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
impl From<FluenceEstimator> for Proptype {
    fn from(value: FluenceEstimator) -> Self {
        Self::FluenceEstimator(value)
    }
}

#[cfg(test)]
mod test {
    use crate::{properties::Proptype, surface::hit_map::fluence_estimator::FluenceEstimator};

    #[test]
    fn fmt() {
        assert_eq!(format!("{}", FluenceEstimator::Voronoi), "Voronoi");
        assert_eq!(format!("{}", FluenceEstimator::KDE), "KDE");
        assert_eq!(format!("{}", FluenceEstimator::Binning), "Binning");
    }
    #[test]
    fn from() {
        assert!(matches!(
            FluenceEstimator::Voronoi.into(),
            Proptype::FluenceEstimator(_)
        ));
    }
}
