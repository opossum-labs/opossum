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
}

impl DampedLeastSquares {
    /// Creates a new DLS solver bound to the given merit function.
    #[must_use]
    pub fn new(merit: MeritFunction) -> Self {
        Self {
            merit,
            initial_damping: 1e-3,
            damping: 1e-3,
            nu: 2.0,
            min_damping: 1e-9,
            max_damping: 1e9,
            gradient_tolerance: 1e-7,
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
}

impl<O> Solver<O, DlsIterState> for DampedLeastSquares {
    fn name(&self) -> &str {
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

        // 1. Compute residuals and parallel Jacobian
        let r = self.merit.evaluate_residuals(&x);
        let j = self.merit.compute_jacobian(&x, &r);
        let n = x.len();

        // 2. Normal equation components: g = J^T * r
        let g = j.transpose() * &r;

        if g.amax() < self.gradient_tolerance {
            return Ok((state, None));
        }

        // A = J^T * J
        let a = j.transpose() * &j;

        // Diagonal Marquardt matrix D with scale safeguard
        let mut d_diag = DVector::zeros(n);
        for i in 0..n {
            d_diag[i] = a[(i, i)].max(1e-6);
        }
        let d = DMatrix::from_diagonal(&d_diag);

        // 3. Inner step resolution: adapt damping until cost decreases
        let mut step_accepted = false;
        let mut best_x_new = x.clone();
        let mut best_cost_new = current_cost;

        for _ in 0..10 {
            // Damped normal equation: (A + lambda * D) * delta_x = -g
            let a_damped = &a + self.damping * &d;

            let delta_x_opt = a_damped
                .clone()
                .cholesky()
                .map(|chol| chol.solve(&(-&g)))
                .or_else(|| a_damped.lu().solve(&(-&g)));

            if let Some(delta_x) = delta_x_opt {
                let x_cand: Vec<f64> = x
                    .iter()
                    .zip(delta_x.iter())
                    .map(|(&xi, &dxi)| xi + dxi)
                    .collect();

                let r_cand = self.merit.evaluate_residuals(&x_cand);
                let cost_cand = r_cand.norm_squared();

                let actual_reduction = current_cost - cost_cand;
                let pred_reduction =
                    -delta_x.dot(&g) - 0.5 * (delta_x.transpose() * &a * &delta_x)[(0, 0)];

                if actual_reduction > 0.0 && pred_reduction > 0.0 {
                    let rho = actual_reduction / pred_reduction;
                    // Marquardt-Nielsen damping adjustment
                    let factor = 1.0 - (2.0 * rho - 1.0).powi(3);
                    self.damping = (self.damping * factor.max(1.0 / 3.0)).max(self.min_damping);
                    self.nu = 2.0;

                    best_x_new = x_cand;
                    best_cost_new = cost_cand;
                    step_accepted = true;
                    break;
                } else {
                    self.damping = (self.damping * self.nu).min(self.max_damping);
                    self.nu *= 2.0;
                }
            } else {
                self.damping = (self.damping * self.nu).min(self.max_damping);
                self.nu *= 2.0;
            }
        }

        if step_accepted {
            Ok((state.param(best_x_new).cost(best_cost_new), None))
        } else {
            Ok((state, None))
        }
    }
}