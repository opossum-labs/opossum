use super::variable::{FiniteDifferenceMethod, FreeVariable};
use crate::error::{OpmResult, OpossumError};
use serde::{Deserialize, Serialize};

/// Supported numerical optimization algorithms and their solver-specific hyperparameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OptimizationAlgorithm {
    /// Damped Least Squares (Levenberg-Marquardt) local non-linear least-squares solver.
    Dls {
        /// Maximum number of solver iterations.
        max_iterations: u64,
        #[allow(clippy::doc_markdown)]
        /// Initial Levenberg-Marquardt damping factor $\lambda_0$.
        initial_damping: f64,
        /// Convergence threshold on the infinity norm of the merit gradient.
        gradient_tolerance: f64,
    },
    /// Nelder-Mead simplex direct search method (derivative-free).
    NelderMead {
        /// Maximum number of simplex iterations.
        max_iterations: u64,
        /// Convergence threshold on the standard deviation of function values across simplex vertices.
        sd_tolerance: f64,
    },
    /// Limited-memory Broyden-Fletcher-Goldfarb-Shanno (quasi-Newton method).
    LBFGS {
        /// Maximum number of quasi-Newton iterations.
        max_iterations: u64,
        /// Convergence threshold on the norm of the approximated gradient.
        gradient_tolerance: f64,
        /// Number of past correction pairs stored to approximate the inverse Hessian.
        history_size: usize,
        /// Finite-difference scheme used to compute numerical gradients.
        gradient_method: FiniteDifferenceMethod,
    },
    /// Adaptive Simulated Annealing global stochastic optimization (Corana 1/5th success rule).
    AdaptiveSimulatedAnnealing {
        /// Maximum number of Markov-chain step evaluations.
        max_iterations: u64,
        #[allow(clippy::doc_markdown)]
        /// Initial annealing temperature $T_0$ (should be comparable to the initial merit cost).
        initial_temperature: f64,
        #[allow(clippy::doc_markdown)]
        /// Geometric cooling factor $\alpha$ in $T_{k+1} = \alpha \cdot T_k$ (typically `0.85..=0.999`).
        cooling_rate: f64,
        /// Number of proposals per variable before adapting step sizes.
        adaptation_cycle: u64,
    },
}

impl Default for OptimizationAlgorithm {
    fn default() -> Self {
        Self::nelder_mead()
    }
}

impl OptimizationAlgorithm {
    /// Creates a default Damped Least Squares (DLS) configuration (50 iterations).
    #[must_use]
    pub const fn dls() -> Self {
        Self::Dls {
            max_iterations: 50,
            initial_damping: 1e-3,
            gradient_tolerance: 1e-7,
        }
    }

    /// Creates a default Nelder-Mead configuration (100 iterations, `1e-6` standard deviation tolerance).
    #[must_use]
    pub const fn nelder_mead() -> Self {
        Self::NelderMead {
            max_iterations: 100,
            sd_tolerance: 1e-6,
        }
    }

    /// Creates a default L-BFGS configuration (60 iterations) using central finite differences.
    #[must_use]
    pub const fn lbfgs() -> Self {
        Self::LBFGS {
            max_iterations: 60,
            gradient_tolerance: 1e-5,
            history_size: 10,
            gradient_method: FiniteDifferenceMethod::Central,
        }
    }

    #[allow(clippy::doc_markdown)]
    /// Creates a default Adaptive Simulated Annealing configuration (10,000 iterations, $T_0 = 100$, $\alpha = 0.995$).
    #[must_use]
    pub const fn adaptive_simulated_annealing() -> Self {
        Self::AdaptiveSimulatedAnnealing {
            max_iterations: 10_000,
            initial_temperature: 100.0,
            cooling_rate: 0.995,
            adaptation_cycle: 20,
        }
    }

    /// Sets the geometric cooling rate $\alpha$ if this variant is [`OptimizationAlgorithm::AdaptiveSimulatedAnnealing`].
    #[must_use]
    pub const fn with_cooling_rate(mut self, rate: f64) -> Self {
        if let Self::AdaptiveSimulatedAnnealing { cooling_rate, .. } = &mut self {
            *cooling_rate = rate;
        }
        self
    }

    #[allow(clippy::doc_markdown)]
    /// Sets the initial temperature $T_0$ if this variant is [`OptimizationAlgorithm::AdaptiveSimulatedAnnealing`].
    #[must_use]
    pub const fn with_temperature(mut self, temp: f64) -> Self {
        if let Self::AdaptiveSimulatedAnnealing {
            initial_temperature,
            ..
        } = &mut self
        {
            *initial_temperature = temp;
        }
        self
    }

    /// Sets the adaptation cycle length if this variant is [`OptimizationAlgorithm::AdaptiveSimulatedAnnealing`].
    #[must_use]
    pub const fn with_adaptation_cycle(mut self, cycle: u64) -> Self {
        if let Self::AdaptiveSimulatedAnnealing {
            adaptation_cycle, ..
        } = &mut self
        {
            *adaptation_cycle = cycle;
        }
        self
    }

    /// Sets the maximum number of solver iterations for any algorithm variant.
    #[must_use]
    pub const fn with_max_iterations(mut self, iters: u64) -> Self {
        match &mut self {
            Self::Dls { max_iterations, .. }
            | Self::NelderMead { max_iterations, .. }
            | Self::LBFGS { max_iterations, .. }
            | Self::AdaptiveSimulatedAnnealing { max_iterations, .. } => *max_iterations = iters,
        }
        self
    }

    /// Sets the standard deviation tolerance for [`OptimizationAlgorithm::NelderMead`]
    /// or the gradient tolerance for [`OptimizationAlgorithm::LBFGS`].
    ///
    /// Has no effect on other algorithm variants.
    #[must_use]
    pub const fn with_sd_tolerance(mut self, tol: f64) -> Self {
        match &mut self {
            Self::NelderMead { sd_tolerance, .. } => *sd_tolerance = tol,
            Self::LBFGS {
                gradient_tolerance, ..
            } => *gradient_tolerance = tol,
            Self::AdaptiveSimulatedAnnealing { .. } | Self::Dls { .. } => {}
        }
        self
    }

    /// Automatically generates a non-degenerate, bound-respecting initial simplex of $N + 1$ vertices
    /// for $N$ free variables starting from `initial_values`.
    ///
    /// Each perturbed vertex shifts one coordinate by 5% of its bound span (or 5% of its magnitude,
    /// falling back to `1e-3` near zero), flipping the step direction if it would exceed `bounds.max`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `free_vars` is empty, or
    /// - `initial_values.len() != free_vars.len()`.
    pub fn build_initial_simplex(
        free_vars: &[FreeVariable],
        initial_values: &[f64],
    ) -> OpmResult<Vec<Vec<f64>>> {
        let n = free_vars.len();
        if n == 0 {
            return Err(OpossumError::Other(
                "Cannot build simplex without free variables".to_string(),
            ));
        }
        if initial_values.len() != n {
            return Err(OpossumError::Other(format!(
                "Expected {n} initial values for simplex construction, got {}",
                initial_values.len()
            )));
        }
        let base_vertex = initial_values.to_vec();
        let mut simplex = Vec::with_capacity(n + 1);
        simplex.push(base_vertex.clone());
        for (i, var) in free_vars.iter().enumerate() {
            let mut vertex = base_vertex.clone();
            let current = vertex[i];
            let step = var.bounds.finite_span().map_or_else(
                || {
                    if current.abs() > 1e-7 {
                        current.abs() * 0.05
                    } else {
                        1e-3
                    }
                },
                |span| span * 0.05,
            );
            let mut candidate = current + step;
            if candidate > var.bounds.max {
                candidate = current - step;
            }
            vertex[i] = candidate;
            simplex.push(vertex);
        }
        Ok(simplex)
    }
}
