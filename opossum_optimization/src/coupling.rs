use serde::{Deserialize, Serialize};

use crate::address::ParameterAddress;
use opossum_core::error::{OpmResult, OpossumError};

/// Specific coupling relationship between parameters[cite: 1].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CouplingType {
    /// Target = Source (e.g., identical air spacings in symmetric setups)
    Identical { source: ParameterAddress },
    /// Target = -Source (e.g., symmetric biconvex lenses with R2 = -R1)
    Opposite { source: ParameterAddress },
    /// Target = Source * factor + offset
    Affine {
        source: ParameterAddress,
        factor: f64,
        offset: f64,
    },
    /// Enforces a fixed total system length
    ///
    /// Target = total_length - sum(other_segments)
    TotalLengthLock {
        total_length: f64,
        other_segments: Vec<ParameterAddress>,
    },
}

/// A coupling rule that drives a dependent parameter from other system variables
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterCoupling {
    /// Dependent parameter being modified[cite: 1].
    pub target: ParameterAddress,
    /// Mathematical rule defining how target is calculated[cite: 1].
    pub rule: CouplingType,
}

impl ParameterCoupling {
    /// Creates an identical parameter coupling[cite: 1].
    pub const fn identical(target: ParameterAddress, source: ParameterAddress) -> Self {
        Self {
            target,
            rule: CouplingType::Identical { source },
        }
    }

    /// Creates an opposite parameter coupling (Target = -Source)[cite: 1].
    pub const fn opposite(target: ParameterAddress, source: ParameterAddress) -> Self {
        Self {
            target,
            rule: CouplingType::Opposite { source },
        }
    }

    /// Creates an affine coupling (Target = factor * Source + offset)[cite: 1].
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

    /// Creates a total length lock coupling[cite: 1].
    pub fn total_length_lock(
        target: ParameterAddress,
        total_length: f64,
        other_segments: Vec<ParameterAddress>,
    ) -> OpmResult<Self> {
        if total_length <= 0.0 {
            return Err(OpossumError::Other(
                "Total length must be positive".to_string(),
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
