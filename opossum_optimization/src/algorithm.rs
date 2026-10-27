use serde::{Deserialize, Serialize};

use crate::variable::FreeVariable;
use opossum_core::error::{OpmResult, OpossumError};

/// Supported numerical optimization algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OptimizationAlgorithm {
    /// Nelder-Mead simplex direct search method (derivative-free).
    NelderMead {
        /// Standard deviation tolerance for convergence.
        sd_tolerance: f64,
    },
    // Placeholder for future Stage 2/3 solvers: LBFGS, LevenbergMarquardt, etc.[cite: 1]
}

impl Default for OptimizationAlgorithm {
    fn default() -> Self {
        Self::NelderMead { sd_tolerance: 1e-6 }
    }
}

impl OptimizationAlgorithm {
    /// Automatically generates a non-degenerate, bound-respecting initial simplex for Nelder-Mead[cite: 1].
    ///
    /// For an $n$-dimensional problem, constructs $n + 1$ vertices in $\mathbb{R}^n$:
    /// - Vertex 0: initial parameter vector $x_0$.
    /// - Vertex $i$: $x_0$ with component $i$ perturbed along its typical scale[cite: 1].
    pub fn build_initial_simplex(free_vars: &[FreeVariable]) -> OpmResult<Vec<Vec<f64>>> {
        let n = free_vars.len();
        if n == 0 {
            return Err(OpossumError::Other(
                "Cannot build simplex without free variables".to_string(),
            ));
        }

        let base_vertex: Vec<f64> = free_vars.iter().map(|v| v.initial_value).collect();
        let mut simplex = Vec::with_capacity(n + 1);

        // First vertex is the initial guess
        simplex.push(base_vertex.clone());

        // Construct remaining n vertices by perturbing each dimension
        for (i, var) in free_vars.iter().enumerate() {
            let mut vertex = base_vertex.clone();
            let current = vertex[i];

            // Determine characteristic step size
            let step = if let Some(bounds) = &var.bounds {
                // Use 5% of the allowable bounding box
                (bounds.max - bounds.min) * 0.05
            } else if current.abs() > 1e-7 {
                // Use 5% of the current scale
                current.abs() * 0.05
            } else {
                // Fallback for near-zero parameters
                1e-3
            };

            // Respect bounds: if stepping forward exceeds upper bound, step backward
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
