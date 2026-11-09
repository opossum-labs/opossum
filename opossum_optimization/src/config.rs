use serde::{Deserialize, Serialize};

use crate::{
    algorithm::OptimizationAlgorithm, coupling::ParameterCoupling, operand::StandardOperand,
    variable::FreeVariable,
};

const fn default_target_cost() -> f64 {
    1e-8
}

/// Fully serializable configuration defining a complete optical optimization setup.
///
/// Can be persisted to `.opt.json` template files or embedded directly
/// in project files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptimizationConfig {
    /// List of independent free parameters tuned by the numerical solver.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub free_variables: Vec<FreeVariable>,

    /// Coupling rules defining deterministic dependencies between parameters[cite: 1].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub couplings: Vec<ParameterCoupling>,

    /// Evaluated merit criteria (targets, inequality bounds, geometry penalties)[cite: 1].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operands: Vec<StandardOperand>,

    /// Mathematical optimization algorithm and its specific hyperparameters[cite: 1].
    #[serde(default)]
    pub algorithm: OptimizationAlgorithm,

    /// Cost threshold triggering early termination once achieved[cite: 1].
    #[serde(default = "default_target_cost")]
    pub target_cost: f64,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            free_variables: Vec::new(),
            couplings: Vec::new(),
            operands: Vec::new(),
            algorithm: OptimizationAlgorithm::default(),
            target_cost: default_target_cost(),
        }
    }
}

impl OptimizationConfig {
    /// Creates a new, empty optimization configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a free variable to the configuration
    #[must_use]
    pub fn with_variable(mut self, variable: FreeVariable) -> Self {
        self.free_variables.push(variable);
        self
    }

    /// Adds a parameter coupling to the configuration
    #[must_use]
    pub fn with_coupling(mut self, coupling: ParameterCoupling) -> Self {
        self.couplings.push(coupling);
        self
    }

    /// Adds an operand to the configuration
    #[must_use]
    pub fn with_operand(mut self, operand: impl Into<StandardOperand>) -> Self {
        self.operands.push(operand.into());
        self
    }

    /// Sets the numerical optimization algorithm
    #[must_use]
    pub const fn with_algorithm(mut self, algorithm: OptimizationAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Sets the target cost threshold
    #[must_use]
    pub const fn with_target_cost(mut self, target_cost: f64) -> Self {
        self.target_cost = target_cost;
        self
    }
}
