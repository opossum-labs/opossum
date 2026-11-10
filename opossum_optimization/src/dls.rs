use argmin::core::{Error, IterState, KV, Problem, Solver, State};
use nalgebra::{DMatrix, DVector};
use opossum_core::error::OpossumError;

use crate::merit::MeritFunction;

pub type DlsIterState = IterState<Vec<f64>, (), (), (), (), f64>;
/// Damped Least Squares (Levenberg-Marquardt) solver implementing the `argmin::core::Solver` trait.
pub struct DampedLeastSquares {
    merit: MeritFunction,
    initial_damping: f64,
    damping: f64,
    nu: f64,
    min_damping: f64,
    max_damping: f64,
    gradient_tolerance: f64,
    /// Maximum allowed Euclidean norm for parameter delta ||dx|| in one iteration.
    max_step_norm: f64,
}

impl DampedLeastSquares {
    /// Creates a new DLS solver bound to the given merit function.
    #[must_use]
    pub const fn new(merit: MeritFunction) -> Self {
        Self {
            merit,
            initial_damping: 1e-3,
            damping: 1e-3,
            nu: 2.0,
            min_damping: 1e-9,
            max_damping: 1e9,
            gradient_tolerance: 1e-7,
            max_step_norm: 5.0, // Prevents wild jumps in curvature/distance space
        }
    }

    /// Sets the initial Marquardt damping parameter lambda.
    #[must_use]
    pub const fn with_initial_damping(mut self, damping: f64) -> Self {
        self.initial_damping = damping;
        self.damping = damping;
        self
    }

    /// Sets the gradient infinity-norm stopping tolerance.
    #[must_use]
    pub const fn with_gradient_tolerance(mut self, tol: f64) -> Self {
        self.gradient_tolerance = tol;
        self
    }
    /// Sets the maximum parameter step norm allowed per iteration.
    #[must_use]
    pub const fn with_max_step_norm(mut self, max_step: f64) -> Self {
        self.max_step_norm = max_step;
        self
    }
}

impl<O> Solver<O, DlsIterState> for DampedLeastSquares {
    fn name(&self) -> &'static str {
        "Damped Least Squares (Levenberg-Marquardt)"
    }

    fn init(
        &mut self,
        _problem: &mut Problem<O>,
        state: DlsIterState,
    ) -> Result<(DlsIterState, Option<KV>), Error> {
        let param = state.get_param().cloned().ok_or_else(|| {
            OpossumError::Other("DampedLeastSquares requires an initial parameter vector".into())
        })?;
        let r0 = self.merit.evaluate_residuals(&param);
        let cost0 = r0.norm_squared();
        self.damping = self.initial_damping;
        self.nu = 2.0;

        Ok((state.param(param).cost(cost0), None))
    }

    fn next_iter(
        &mut self,
        _problem: &mut Problem<O>,
        state: DlsIterState,
    ) -> Result<(DlsIterState, Option<KV>), Error> {
        let x = state.get_param().cloned().ok_or_else(|| {
            OpossumError::Other("Missing current parameter vector in DLS state".into())
        })?;
        let current_cost = state.get_cost();

        // 1. Compute residuals and parallel Jacobian (central differences)
        let r = self.merit.evaluate_residuals(&x);
        let j = self.merit.compute_jacobian(&x, &r);
        let n = x.len();

        // 2. Gradient vector: g = J^T * r
        let g = j.transpose() * &r;

        if g.amax() < self.gradient_tolerance {
            return Ok((state, None));
        }

        // Hessian approximation: A = J^T * J
        let a = j.transpose() * &j;

        // Diagonal Marquardt matrix D with scale safeguard
        let mut d_diag = DVector::zeros(n);
        for i in 0..n {
            d_diag[i] = a[(i, i)].max(1e-6);
        }
        let d = DMatrix::from_diagonal(&d_diag);

        // 3. Inner loop: test step candidates
        let mut step_accepted = false;
        let mut best_x_new = x.clone();
        let mut best_cost_new = current_cost;

        for _ in 0..12 {
            // (A + lambda * D) * delta_x = -g
            let a_damped = &a + self.damping * &d;

            let delta_x_opt = a_damped
                .clone()
                .cholesky()
                .map(|chol| chol.solve(&(-&g)))
                .or_else(|| a_damped.lu().solve(&(-&g)));

            if let Some(mut delta_x) = delta_x_opt {
                // Step size clamping: prevent overshooting the validity region
                let step_norm = delta_x.norm();
                if step_norm > self.max_step_norm {
                    delta_x *= self.max_step_norm / step_norm;
                }

                let x_cand: Vec<f64> = x
                    .iter()
                    .zip(delta_x.iter())
                    .map(|(&xi, &dxi)| xi + dxi)
                    .collect();

                let r_cand = self.merit.evaluate_residuals(&x_cand);
                let cost_cand = r_cand.norm_squared();

                let actual_reduction = current_cost - cost_cand;
                let pred_reduction = 0.5f64.mul_add(
                    -(delta_x.transpose() * &a * &delta_x)[(0, 0)],
                    -delta_x.dot(&g),
                );

                if actual_reduction > 0.0 {
                    let rho = if pred_reduction.abs() > 1e-12 {
                        actual_reduction / pred_reduction
                    } else {
                        1.0
                    };

                    // Adaptive damping update (Marquardt-Nielsen)
                    let factor = (1.0 - (2.0f64.mul_add(rho, -1.0)).powi(3)).clamp(0.1, 0.5);
                    self.damping = (self.damping * factor).max(self.min_damping);
                    self.nu = 2.0;

                    best_x_new = x_cand;
                    best_cost_new = cost_cand;
                    step_accepted = true;
                    break;
                }
                // Step rejected: increase damping and contract search radius
                self.damping = (self.damping * self.nu).min(self.max_damping);
                self.nu *= 2.0;
            } else {
                self.damping = (self.damping * self.nu).min(self.max_damping);
                self.nu *= 2.0;
            }
        }

        if step_accepted {
            Ok((state.param(best_x_new).cost(best_cost_new), None))
        } else {
            // Force progress with small gradient-descent step if trapped in a saddle
            let grad_step: Vec<f64> = x
                .iter()
                .zip(g.iter())
                .map(|(&xi, &gi)| (1e-4 / (self.damping + 1.0)).mul_add(-gi, xi))
                .collect();
            let cost_forced = self.merit.evaluate_residuals(&grad_step).norm_squared();
            if cost_forced < current_cost {
                Ok((state.param(grad_step).cost(cost_forced), None))
            } else {
                Ok((state, None))
            }
        }
    }
}
