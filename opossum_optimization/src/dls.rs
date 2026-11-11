use crate::merit::MeritFunction;
use argmin::core::{Error, IterState, KV, Problem, Solver, State};
use nalgebra::{DMatrix, DVector};
use opossum_core::error::OpossumError;

pub type DlsIterState = IterState<Vec<f64>, (), (), (), (), f64>;

/// Robust Damped Least Squares (Levenberg-Marquardt) solver featuring
/// diagonal parameter scaling and graceful damping recovery.
pub struct DampedLeastSquares {
    merit: MeritFunction,
    initial_damping: f64,
    damping: f64,
    nu: f64,
    min_damping: f64,
    max_damping: f64,
    gradient_tolerance: f64,
    /// Maximum allowed parameter step relative to variable bounds
    max_step_norm: f64,
}

impl DampedLeastSquares {
    /// Creates a new DLS solver bound to the given merit function.
    #[must_use]
    pub const fn new(merit: MeritFunction) -> Self {
        Self {
            merit,
            initial_damping: 1e-2,
            damping: 1e-2,
            nu: 2.0,
            min_damping: 1e-8,
            max_damping: 1e6,
            gradient_tolerance: 1e-6,
            max_step_norm: 0.25, // Restrict step to 25% of parameter scale per iteration
        }
    }

    #[must_use]
    pub const fn with_initial_damping(mut self, damping: f64) -> Self {
        self.initial_damping = damping;
        self.damping = damping;
        self
    }

    #[must_use]
    pub const fn with_gradient_tolerance(mut self, tol: f64) -> Self {
        self.gradient_tolerance = tol;
        self
    }

    #[must_use]
    pub const fn with_max_step_norm(mut self, max_step: f64) -> Self {
        self.max_step_norm = max_step;
        self
    }

    /// Computes diagonal scaling factors based on variable bounds or typical ranges.
    fn parameter_scales(&self) -> Vec<f64> {
        self.merit
            .free_variables()
            .iter()
            .map(|var| {
                if let Some(bounds) = &var.bounds {
                    (bounds.max - bounds.min).abs().max(1e-4)
                } else {
                    1.0
                }
            })
            .collect()
    }
}

impl<O> Solver<O, DlsIterState> for DampedLeastSquares {
    fn name(&self) -> &'static str {
        "Damped Least Squares (Normalized Levenberg-Marquardt)"
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
        let scales = self.parameter_scales();
        let n = x.len();

        // 1. Evaluate residuals and Jacobian
        let r = self.merit.evaluate_residuals(&x);
        let mut j = self.merit.compute_jacobian(&x, &r);

        // 2. Scale Jacobian columns by parameter natural spans: J_scaled = J * diag(scales)
        for col in 0..n {
            let scale = scales[col];
            for row in 0..j.nrows() {
                j[(row, col)] *= scale;
            }
        }

        // Gradient in scaled coordinates: g_scaled = J_scaled^T * r
        let g = j.transpose() * &r;
        if g.amax() < self.gradient_tolerance {
            return Ok((state, None));
        }

        // Approximate Hessian in scaled coordinates: A = J_scaled^T * J_scaled
        let a = j.transpose() * &j;

        // Diagonal Marquardt damping matrix with strict lower conditioning bound
        let mut d_diag = DVector::zeros(n);
        for i in 0..n {
            d_diag[i] = a[(i, i)].max(1e-3);
        }
        let d = DMatrix::from_diagonal(&d_diag);

        // 3. Search for acceptable trust-region step
        let mut step_accepted = false;
        let mut best_x_new = x.clone();
        let mut best_cost_new = current_cost;

        for _ in 0..10 {
            let a_damped = &a + self.damping * &d;

            // Solve (A + lambda * D) * delta_scaled = -g
            let delta_scaled_opt = a_damped
                .clone()
                .cholesky()
                .map(|chol| chol.solve(&(-&g)))
                .or_else(|| a_damped.lu().solve(&(-&g)));

            if let Some(mut delta_scaled) = delta_scaled_opt {
                // Restrict maximum step in scaled space
                let norm = delta_scaled.norm();
                if norm > self.max_step_norm {
                    delta_scaled *= self.max_step_norm / norm;
                }

                // Transform back to physical units: delta_phys = delta_scaled * scale
                let x_cand: Vec<f64> = x
                    .iter()
                    .enumerate()
                    .map(|(i, &xi)| xi + delta_scaled[i] * scales[i])
                    .collect();

                let r_cand = self.merit.evaluate_residuals(&x_cand);
                let cost_cand = r_cand.norm_squared();
                let actual_reduction = current_cost - cost_cand;

                let pred_reduction = 0.5f64.mul_add(
                    -(delta_scaled.transpose() * &a * &delta_scaled)[(0, 0)],
                    -delta_scaled.dot(&g),
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

                // Reject step: Increase damping
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
            // Recovery: Reset damping to a moderate value to prevent stall in subsequent iterations
            self.damping = (self.initial_damping * 10.0).min(self.max_damping);
            self.nu = 2.0;

            // Fall back to a tiny gradient step in scaled space
            let grad_step: Vec<f64> = x
                .iter()
                .enumerate()
                .map(|(i, &xi)| {
                    let dxi_scaled = -1e-3 * g[i] / (g.norm() + 1e-12);
                    xi + dxi_scaled * scales[i]
                })
                .collect();

            let cost_forced = self.merit.evaluate_residuals(&grad_step).norm_squared();
            if cost_forced < current_cost {
                Ok((state.param(grad_step).cost(cost_forced), None))
            } else {
                // No progress possible at current point
                Ok((state, None))
            }
        }
    }
}
