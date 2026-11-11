use super::variable::{FiniteDifferenceMethod, FreeVariable};
use crate::error::{OpmResult, OpossumError};
use serde::{Deserialize, Serialize};

/// Supported numerical optimization algorithms and their solver-specific controls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OptimizationAlgorithm {
    /// Damped Least Squares (Levenberg-Marquardt) optimization.
    Dls {
        max_iterations: u64,
        initial_damping: f64,
        gradient_tolerance: f64,
    },
    /// Nelder-Mead simplex direct search method (derivative-free).
    NelderMead {
        max_iterations: u64,
        sd_tolerance: f64,
    },
    /// Limited-memory Broyden-Fletcher-Goldfarb-Shanno (quasi-Newton method).
    LBFGS {
        max_iterations: u64,
        gradient_tolerance: f64,
        history_size: usize,
        gradient_method: FiniteDifferenceMethod,
    },
    /// Simulated Annealing global stochastic optimization.
    SimulatedAnnealing {
        max_iterations: u64,
        /// Initial temperature T0.
        initial_temperature: f64,
        /// Geometric cooling rate alpha in `T_{k+1} = alpha * T_k` (e.g. 0.85 .. 0.99).
        cooling_rate: f64,
        stall_best: u64,
        stall_accepted: u64,
        reannealing_fixed: Option<u64>,
    },
}

impl Default for OptimizationAlgorithm {
    fn default() -> Self {
        Self::nelder_mead()
    }
}

impl OptimizationAlgorithm {
    /// Creates a default DLS configuration.
    #[must_use]
    pub const fn dls() -> Self {
        Self::Dls {
            max_iterations: 50,
            initial_damping: 1e-3,
            gradient_tolerance: 1e-7,
        }
    }
    /// Creates a default Nelder-Mead configuration (100 iterations, 1e-6 tolerance).
    #[must_use]
    pub const fn nelder_mead() -> Self {
        Self::NelderMead {
            max_iterations: 100,
            sd_tolerance: 1e-6,
        }
    }

    /// Creates a default L-BFGS configuration with central differences.
    #[must_use]
    pub const fn lbfgs() -> Self {
        Self::LBFGS {
            max_iterations: 60,
            gradient_tolerance: 1e-5,
            history_size: 10,
            gradient_method: FiniteDifferenceMethod::Central,
        }
    }

    /// Creates a default Simulated Annealing configuration.
    #[must_use]
    pub const fn simulated_annealing() -> Self {
        Self::SimulatedAnnealing {
            max_iterations: 10_000,
            initial_temperature: 100.0, // Should be roughly comparable to initial merit cost
            cooling_rate: 0.995,        // Slow geometric cooling
            stall_best: u64::MAX, // Do not abort early just because no new global best was found
            stall_accepted: 1_000, // Terminate only after 1000 consecutive rejected steps
            reannealing_fixed: None,
        }
    }
    /// Sets the cooling rate for Simulated Annealing.
    #[must_use]
    pub const fn with_cooling_rate(mut self, rate: f64) -> Self {
        if let Self::SimulatedAnnealing { cooling_rate, .. } = &mut self {
            *cooling_rate = rate;
        }
        self
    }
    /// Sets the initial temperature for Simulated Annealing.
    #[must_use]
    pub const fn with_temperature(mut self, temp: f64) -> Self {
        if let Self::SimulatedAnnealing {
            initial_temperature,
            ..
        } = &mut self
        {
            *initial_temperature = temp;
        }
        self
    }

    #[must_use]
    pub const fn with_max_iterations(mut self, iters: u64) -> Self {
        match &mut self {
            Self::Dls { max_iterations, .. }
            | Self::NelderMead { max_iterations, .. }
            | Self::LBFGS { max_iterations, .. }
            | Self::SimulatedAnnealing { max_iterations, .. } => *max_iterations = iters,
        }
        self
    }

    /// Sets the standard deviation tolerance for Nelder-Mead or gradient tolerance for L-BFGS.
    #[must_use]
    pub const fn with_sd_tolerance(mut self, tol: f64) -> Self {
        match &mut self {
            Self::NelderMead { sd_tolerance, .. } => *sd_tolerance = tol,
            Self::LBFGS {
                gradient_tolerance, ..
            } => *gradient_tolerance = tol,
            Self::SimulatedAnnealing { .. } | Self::Dls { .. } => {}
        }
        self
    }

    /// Automatically generates a non-degenerate, bound-respecting initial simplex.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// - no free variable were given
    /// - a free variable has no start value
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
                        v.address
                    ))
                })
            })
            .collect::<Result<_, _>>()?;

        let mut simplex = Vec::with_capacity(n + 1);
        simplex.push(base_vertex.clone());

        for (i, var) in free_vars.iter().enumerate() {
            let mut vertex = base_vertex.clone();
            let current = vertex[i];
            let step = var.bounds.as_ref().map_or_else(
                || {
                    if current.abs() > 1e-7 {
                        current.abs() * 0.05
                    } else {
                        1e-3
                    }
                },
                |bounds| (bounds.max - bounds.min) * 0.05,
            );

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
