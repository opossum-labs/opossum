use crate::variable::FreeVariable;
use opossum_core::error::{OpmResult, OpossumError};
use serde::{Deserialize, Serialize};

/// Supported numerical optimization algorithms and their solver-specific controls[cite: 1].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OptimizationAlgorithm {
    /// Nelder-Mead simplex direct search method[cite: 1].
    NelderMead {
        /// Maximum number of simplex iterations.
        max_iterations: u64,
        /// Standard deviation tolerance across simplex vertices for termination[cite: 1].
        sd_tolerance: f64,
    },
    // Future: SimulatedAnnealing { max_steps: u64, initial_temp: f64, cooling_factor: f64 },
    // Future: LBFGS { max_iterations: u64, gradient_tolerance: f64 },
}

impl Default for OptimizationAlgorithm {
    fn default() -> Self {
        Self::nelder_mead()
    }
}

impl OptimizationAlgorithm {
    /// Creates a default Nelder-Mead configuration (100 iterations, 1e-6 tolerance)[cite: 1].
    pub fn nelder_mead() -> Self {
        Self::NelderMead {
            max_iterations: 100,
            sd_tolerance: 1e-6,
        }
    }

    /// Sets the maximum iteration count for algorithms supporting it[cite: 1].
    pub fn with_max_iterations(mut self, iters: u64) -> Self {
        match &mut self {
            Self::NelderMead { max_iterations, .. } => *max_iterations = iters,
        }
        self
    }

    /// Sets the standard deviation tolerance for Nelder-Mead.
    pub fn with_sd_tolerance(mut self, tol: f64) -> Self {
        match &mut self {
            Self::NelderMead { sd_tolerance, .. } => *sd_tolerance = tol,
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
            if let Some(bounds) = &var.bounds {
                if candidate > bounds.max {
                    candidate = current - step;
                }
            }

            vertex[i] = candidate;
            simplex.push(vertex);
        }

        Ok(simplex)
    }
}
