use super::address::ParameterAddress;
use crate::error::{OpmResult, OpossumError};
use serde::{Deserialize, Serialize};
/// Specific deterministic relationship between a dependent target parameter and one or more source parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CouplingType {
    /// Enforces equality: $\text{target} = \text{source}$ (e.g., identical air spacings in symmetric setups).
    Identical {
        /// The source parameter controlling the coupling.
        source: ParameterAddress,
    },
    #[allow(clippy::doc_markdown)]
    /// Enforces sign inversion: $\text{target} = -\text{source}$ (e.g., equiconvex lenses with $R_2 = -R_1$).
    Opposite {
        /// The source parameter controlling the coupling.
        source: ParameterAddress,
    },
    #[allow(clippy::doc_markdown)]
    /// Enforces a linear transformation: $\text{target} = \text{factor} \cdot \text{source} + \text{offset}$.
    Affine {
        /// The source parameter controlling the coupling.
        source: ParameterAddress,
        /// Multiplicative scaling factor.
        factor: f64,
        /// Additive offset in SI base units of the target parameter.
        offset: f64,
    },
    /// Enforces a fixed total system length by adjusting the target distance:
    /// $\text{target} = \text{total\_length} - \sum \text{other\_segments}$.
    TotalLengthLock {
        /// The targeted total optical/geometric path length in meters.
        total_length: f64,
        /// All other distance parameters whose values are subtracted from `total_length`.
        other_segments: Vec<ParameterAddress>,
    },
}
/// A coupling rule that drives a dependent parameter from other system variables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterCoupling {
    /// Dependent parameter being modified.
    pub target: ParameterAddress,
    /// Mathematical rule defining how `target` is calculated.
    pub rule: CouplingType,
}
impl ParameterCoupling {
    /// Creates an identical parameter coupling ($\text{target} = \text{source}$).
    #[must_use]
    pub const fn identical(target: ParameterAddress, source: ParameterAddress) -> Self {
        Self {
            target,
            rule: CouplingType::Identical { source },
        }
    }
    /// Creates an opposite parameter coupling ($\text{target} = -\text{source}$).
    #[must_use]
    pub const fn opposite(target: ParameterAddress, source: ParameterAddress) -> Self {
        Self {
            target,
            rule: CouplingType::Opposite { source },
        }
    }
    /// Creates an affine coupling ($\text{target} = \text{factor} \cdot \text{source} + \text{offset}$).
    #[must_use]
    pub const fn affine(
        target: ParameterAddress,
        source: ParameterAddress,
        factor: f64,
        offset: f64,
    ) -> Self {
        Self {
            target,
            rule: CouplingType::Affine {
                source,
                factor,
                offset,
            },
        }
    }
    /// Creates a total length lock coupling ($\text{target} = \text{total\_length} - \sum \text{other\_segments}$).
    ///
    /// # Errors
    ///
    /// Returns an error if `total_length` is `<= 0.0` or not finite (infinite or `NaN`).
    pub fn total_length_lock(
        target: ParameterAddress,
        total_length: f64,
        other_segments: Vec<ParameterAddress>,
    ) -> OpmResult<Self> {
        if total_length <= 0.0 || !total_length.is_finite() {
            return Err(OpossumError::Other(
                "Total length must be positive and finite".to_string(),
            ));
        }
        Ok(Self {
            target,
            rule: CouplingType::TotalLengthLock {
                total_length,
                other_segments,
            },
        })
    }
}
