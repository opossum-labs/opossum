use crate::address::ParameterAddress;
use std::collections::HashMap;

/// Comprehensive result of an executed optimization process[cite: 1].
#[derive(Debug, Clone)]
pub struct OptimizationSummary {
    /// Cost before optimization Φ(x₀)[cite: 1].
    pub initial_cost: f64,
    /// Final merit cost Φ(x*)[cite: 1].
    pub final_cost: f64,
    /// Number of executed iterations.
    pub iterations: u64,
    /// Indicates whether the optimizer achieved its target cost or convergence criteria[cite: 1].
    pub converged: bool,
    /// Map containing all optimized parameter values (free variables and coupled parameters)[cite: 1].
    pub optimized_parameters: HashMap<ParameterAddress, f64>,
}

impl OptimizationSummary {
    /// Queries the final value of a parameter by its address[cite: 1].
    #[must_use]
    pub fn value(&self, address: &ParameterAddress) -> Option<f64> {
        self.optimized_parameters.get(address).copied()
    }

    /// Relative improvement achieved: (initial - final) / initial.
    #[must_use]
    pub fn relative_improvement(&self) -> f64 {
        if self.initial_cost.abs() < f64::EPSILON {
            0.0
        } else {
            (self.initial_cost - self.final_cost) / self.initial_cost
        }
    }
}
