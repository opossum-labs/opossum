use crate::variable::{FiniteDifferenceMethod, FreeVariable};
use opossum_core::error::{OpmResult, OpossumError};
use serde::{Deserialize, Serialize};

/// Supported numerical optimization algorithms and their solver-specific controls[cite: 1].
/// Supported numerical optimization algorithms and their solver-specific controls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OptimizationAlgorithm {
    /// Nelder-Mead simplex direct search method (derivative-free).
    NelderMead {
        max_iterations: u64,
        sd_tolerance: f64,
    },
    /// Limited-memory Broyden-Fletcher-Goldfarb-Shanno (quasi-Newton method)[cite: 1].
    LBFGS {
        max_iterations: u64,
        gradient_tolerance: f64,
        history_size: usize,
        gradient_method: FiniteDifferenceMethod,
    },
    /// Simulated Annealing global stochastic optimization.
    SimulatedAnnealing {
        /// Initial temperature[cite: 1].
        initial_temperature: f64,
        /// Maximum iterations without global best improvement[cite: 1].
        stall_best: u64,
        /// Maximum iterations without any accepted move[cite: 1].
        stall_accepted: u64,
        /// Periodic reannealing interval (steps). If None, no reannealing occurs.
        reannealing_fixed: Option<u64>,
    },
}

impl Default for OptimizationAlgorithm {
    fn default() -> Self {
        Self::nelder_mead()
    }
}

impl OptimizationAlgorithm {
    /// Creates a default Nelder-Mead configuration (100 iterations, 1e-6 tolerance)[cite: 1].
    pub const fn nelder_mead() -> Self {
        Self::NelderMead {
            max_iterations: 100,
            sd_tolerance: 1e-6,
        }
    }

    /// Creates a default L-BFGS configuration with central differences[cite: 1].
    pub fn lbfgs() -> Self {
        Self::LBFGS {
            max_iterations: 60,
            gradient_tolerance: 1e-5,
            history_size: 10,
            gradient_method: FiniteDifferenceMethod::Central,
        }
    }

    /// Creates a default Simulated Annealing configuration with robust defaults[cite: 1].
    pub fn simulated_annealing() -> Self {
        Self::SimulatedAnnealing {
            initial_temperature: 15.0,
            stall_best: 60,
            stall_accepted: 120,
            reannealing_fixed: None,
        }
    }

    /// Sets the initial temperature for Simulated Annealing[cite: 1].
    pub fn with_temperature(mut self, temp: f64) -> Self {
        if let Self::SimulatedAnnealing { initial_temperature, .. } = &mut self {
            *initial_temperature = temp;
        }
        self
    }

    /// Sets the maximum iteration count.
    pub fn with_max_iterations(mut self, iters: u64) -> Self {
        match &mut self {
            Self::NelderMead { max_iterations, .. } => *max_iterations = iters,
            Self::LBFGS { max_iterations, .. } => *max_iterations = iters,
            _ => {}
        }
        self
    }

    /// Sets the standard deviation tolerance for Nelder-Mead.
    pub const fn with_sd_tolerance(mut self, tol: f64) -> Self {
        match &mut self {
            Self::NelderMead { sd_tolerance, .. } => *sd_tolerance = tol,
            Self::LBFGS { gradient_tolerance, .. } => *gradient_tolerance = tol,
            _ => {}
        }
        self
    }

    /// Automatically generates a non-degenerate, bound-respecting initial simplex[cite: 1].
    pub fn build_initial_simplex(free_vars: &[FreeVariable]) -> OpmResult<Vec<Vec<f64>>> {
        let n = free_vars.len();
        if n == 0 {
            return Err(OpossumError::Other(
                "Cannot build simplex without free variables".to_string(),
            ));
        }

        let base_vertex: Vec<f64> = free_vars
            .iter()
            .map(|v| {
                v.initial_value.ok_or_else(|| {
                    OpossumError::Other(format!(
                        "FreeVariable '{}' has uninitialized start value",
                        v.name
                    ))
                })
            })
            .collect::<Result<_, _>>()?;

        let mut simplex = Vec::with_capacity(n + 1);
        simplex.push(base_vertex.clone());

        for (i, var) in free_vars.iter().enumerate() {
            let mut vertex = base_vertex.clone();
            let current = vertex[i];

            let step = if let Some(bounds) = &var.bounds {
                (bounds.max - bounds.min) * 0.05
            } else if current.abs() > 1e-7 {
                current.abs() * 0.05
            } else {
                1e-3
            };

            let mut candidate = current + step;
            if let Some(bounds) = &var.bounds
                && candidate > bounds.max
            {
                candidate = current - step;
            }

            vertex[i] = candidate;
            simplex.push(vertex);
        }

        Ok(simplex)
    }
}
