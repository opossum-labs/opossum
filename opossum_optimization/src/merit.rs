use argmin::{
    core::{CostFunction, Error, Gradient},
    solver::simulatedannealing::Anneal,
};
use nalgebra::{DMatrix, DVector};
use rand::RngExt;
use std::sync::Arc;

use crate::{
    applier::ParameterApplier,
    coupling::ParameterCoupling,
    operand::{EvaluationContext, Operand},
    resolver::ParameterResolver,
    variable::{FiniteDifferenceMethod, FreeVariable},
};
use opossum_core::{error::OpmResult, opm_document::OpmDocument};
use rayon::prelude::*;
/// Default penalty applied for fatal simulation failures (e.g., ray tracing errors)
pub const DEFAULT_FATAL_PENALTY: f64 = 1.0e12;

/// Default penalty weight scaling parameter box bound violations.
///
/// Set to 1.0e8 to reliably dominate large initial optical merit costs and
/// prevent solvers (especially quasi-Newton line searches) from escaping bounds.
pub const DEFAULT_BOUNDS_PENALTY_WEIGHT: f64 = 1.0e8;

/// Central cost function evaluator $\Phi(x)$ managing blueprint cloning, parameter mapping,
/// simulation execution, and operand evaluation.
#[derive(Clone)]
pub struct MeritFunction {
    blueprint: OpmDocument,
    free_variables: Vec<FreeVariable>,
    couplings: Vec<ParameterCoupling>,
    operands: Vec<Arc<dyn Operand>>,
    fatal_penalty: f64,
    bounds_penalty_weight: f64,
    gradient_method: FiniteDifferenceMethod,
}

impl MeritFunction {
    /// Creates a new `MeritFunction` instance.
    ///
    /// # Errors
    /// Returns an error if the parameter and coupling configuration contains duplicates or cycles.
    pub fn new(
        blueprint: OpmDocument,
        free_variables: Vec<FreeVariable>,
        couplings: Vec<ParameterCoupling>,
        operands: Vec<Arc<dyn Operand>>,
    ) -> OpmResult<Self> {
        ParameterResolver::validate_configuration(&free_variables, &couplings)?;

        // Pre-flight validation of all operands against the initial blueprint model
        for operand in &operands {
            operand.validate(&blueprint)?;
        }

        Ok(Self {
            blueprint,
            free_variables,
            couplings,
            operands,
            fatal_penalty: DEFAULT_FATAL_PENALTY,
            bounds_penalty_weight: DEFAULT_BOUNDS_PENALTY_WEIGHT,
            gradient_method: FiniteDifferenceMethod::default(),
        })
    }

    /// Configures the finite difference approximation method for gradients.
    #[must_use]
    pub const fn with_gradient_method(mut self, method: FiniteDifferenceMethod) -> Self {
        self.gradient_method = method;
        self
    }

    /// Sets a custom penalty value for fatal simulation crashes
    #[must_use]
    pub const fn with_fatal_penalty(mut self, penalty: f64) -> Self {
        self.fatal_penalty = penalty;
        self
    }

    /// Sets a custom penalty weight for box bound violations
    #[must_use]
    pub const fn with_bounds_penalty_weight(mut self, weight: f64) -> Self {
        self.bounds_penalty_weight = weight;
        self
    }

    /// Returns the registered free variables.
    #[must_use]
    pub fn free_variables(&self) -> &[FreeVariable] {
        &self.free_variables
    }

    /// Returns the registered parameter couplings
    #[must_use]
    pub fn couplings(&self) -> &[ParameterCoupling] {
        &self.couplings
    }

    /// Evaluates the total merit value $\Phi(x)$ for candidate vector `x`
    ///
    /// Adheres to the strict no-panic guarantee: any simulation failure or unresolvable
    /// metric is caught and converted into a large penalty cost.
    /// Evaluates the scalar cost Phi(x).
    #[must_use]
    pub fn evaluate(&self, x: &[f64]) -> f64 {
        let mut total_cost = 0.0;
        for (var, &val) in self.free_variables.iter().zip(x.iter()) {
            if let Some(bounds) = &var.bounds {
                let violation = bounds.penalty(val);
                if violation > 0.0 {
                    total_cost = self.bounds_penalty_weight.mul_add(violation, total_cost);
                }
            }
        }

        let mut working_doc = self.blueprint.clone();

        let Ok(parameter_map) =
            ParameterResolver::resolve(&self.free_variables, &self.couplings, x)
        else {
            return self.fatal_penalty;
        };

        for (address, value) in parameter_map {
            if ParameterApplier::set_value(&mut working_doc, &address, value).is_err() {
                return self.fatal_penalty;
            }
        }

        if ParameterApplier::finalize_update(&mut working_doc).is_err() {
            return self.fatal_penalty;
        }

        if working_doc.analyze_headless().is_err() {
            return self.fatal_penalty;
        }

        let context = EvaluationContext::new(&working_doc);
        for operand in &self.operands {
            match operand.evaluate(&context) {
                Ok(operand_cost) => {
                    total_cost += operand_cost;
                }
                Err(err) => {
                    log::warn!("Operand '{}' failed to evaluate: {err}", operand.name());
                    total_cost = self.fatal_penalty.mul_add(0.1, total_cost);
                }
            }
        }
        total_cost
    }
    /// Computes the numerical gradient vector using adaptive finite differences.
    ///
    /// Evaluates parameter perturbations in parallel across available CPU cores via Rayon.
    #[must_use]
    pub fn compute_gradient(&self, x: &[f64]) -> Vec<f64> {
        let n = self.free_variables.len();

        match self.gradient_method {
            FiniteDifferenceMethod::Central => (0..n)
                .into_par_iter()
                .map(|i| {
                    let h = self.free_variables[i].finite_difference_step(x[i]);

                    let mut x_plus = x.to_vec();
                    x_plus[i] += h;

                    let mut x_minus = x.to_vec();
                    x_minus[i] -= h;

                    let f_plus = self.evaluate(&x_plus);
                    let f_minus = self.evaluate(&x_minus);

                    (f_plus - f_minus) / (2.0 * h)
                })
                .collect(),
            FiniteDifferenceMethod::Forward => {
                let f_base = self.evaluate(x);
                (0..n)
                    .into_par_iter()
                    .map(|i| {
                        let h = self.free_variables[i].finite_difference_step(x[i]);

                        let mut x_plus = x.to_vec();
                        x_plus[i] += h;

                        let f_plus = self.evaluate(&x_plus);
                        (f_plus - f_base) / h
                    })
                    .collect()
            }
        }
    }
    /// Evaluates the signed residual vector R(x) for all operands and box bounds.
    ///
    /// The sum of squares ||R(x)||^2 strictly equals the scalar cost Phi(x).
    pub fn evaluate_residuals(&self, x: &[f64]) -> DVector<f64> {
        let n_vars = self.free_variables.len();
        let n_ops = self.operands.len();
        let mut residuals = Vec::with_capacity(n_ops + n_vars);

        // 1. Box bounds penalty residuals
        let mut bound_residuals = Vec::with_capacity(n_vars);
        for (var, &val) in self.free_variables.iter().zip(x.iter()) {
            let res = if let Some(bounds) = &var.bounds {
                if val < bounds.min {
                    (bounds.min - val) * self.bounds_penalty_weight.sqrt()
                } else if val > bounds.max {
                    (val - bounds.max) * self.bounds_penalty_weight.sqrt()
                } else {
                    0.0
                }
            } else {
                0.0
            };
            bound_residuals.push(res);
        }

        // 2. Clone blueprint and apply candidate parameters
        let mut working_doc = self.blueprint.clone();
        let parameter_map = match ParameterResolver::resolve(&self.free_variables, &self.couplings, x) {
            Ok(map) => map,
            Err(_) => {
                let fallback_val = (self.fatal_penalty / (n_ops + n_vars) as f64).sqrt();
                return DVector::from_element(n_ops + n_vars, fallback_val);
            }
        };

        for (address, value) in parameter_map {
            if ParameterApplier::set_value(&mut working_doc, &address, value).is_err() {
                let fallback_val = (self.fatal_penalty / (n_ops + n_vars) as f64).sqrt();
                return DVector::from_element(n_ops + n_vars, fallback_val);
            }
        }

        if ParameterApplier::finalize_update(&mut working_doc).is_err()
            || working_doc.analyze_headless().is_err()
        {
            let fallback_val = (self.fatal_penalty / (n_ops + n_vars) as f64).sqrt();
            return DVector::from_element(n_ops + n_vars, fallback_val);
        }

        // 3. Evaluate individual operand residuals
        let context = EvaluationContext::new(&working_doc);
        for operand in &self.operands {
            match operand.evaluate(&context) {
                Ok(cost) => {
                    residuals.push(cost.max(0.0).sqrt());
                }
                Err(_) => {
                    residuals.push((self.fatal_penalty * 0.1).sqrt());
                }
            }
        }

        // 4. Append bound violation residuals
        residuals.extend(bound_residuals);

        DVector::from_vec(residuals)
    }

    /// Computes the m x n Jacobian matrix J_ij = d(r_i) / d(x_j) using parallel finite differences.
    pub fn compute_jacobian(&self, x: &[f64], base_residuals: &DVector<f64>) -> DMatrix<f64> {
        let n = self.free_variables.len();
        let m = base_residuals.len();

        let columns: Vec<DVector<f64>> = (0..n)
            .into_par_iter()
            .map(|j| {
                let val = x[j];
                // Relative finite difference step size with absolute lower guard
                let h = (1e-4 * val.abs()).max(1e-6);

                let mut x_perturbed = x.to_vec();
                x_perturbed[j] += h;

                let r_perturbed = self.evaluate_residuals(&x_perturbed);
                (r_perturbed - base_residuals) / h
            })
            .collect();

        let mut jacobian = DMatrix::zeros(m, n);
        for (j, col) in columns.into_iter().enumerate() {
            jacobian.set_column(j, &col);
        }
        jacobian
    }
}

impl CostFunction for MeritFunction {
    type Param = Vec<f64>;
    type Output = f64;

    fn cost(&self, param: &Self::Param) -> Result<Self::Output, Error> {
        Ok(self.evaluate(param))
    }
}

impl Gradient for MeritFunction {
    type Param = Vec<f64>;
    type Gradient = Vec<f64>;

    fn gradient(&self, param: &Self::Param) -> Result<Self::Gradient, Error> {
        Ok(self.compute_gradient(param))
    }
}

impl Anneal for MeritFunction {
    type Param = Vec<f64>;
    type Output = Vec<f64>;
    type Float = f64;

    fn anneal(&self, param: &Self::Param, temp: Self::Float) -> Result<Self::Output, Error> {
        let mut rng = rand::rng();
        let mut next_param = param.clone();

        // Scale perturbation by temperature
        let temp_factor = temp.clamp(1e-4, 1.0);

        for (i, var) in self.free_variables.iter().enumerate() {
            let range = var.bounds.as_ref().map_or_else(
                || {
                    if param[i].abs() > 1e-7 {
                        param[i].abs() * 0.5
                    } else {
                        1e-2
                    }
                },
                |bounds| bounds.max - bounds.min,
            );

            // Uniform displacement in interval [-1.0, 1.0]
            let offset = rng.random_range(-1.0..=1.0);
            let step = offset * range * temp_factor;
            let mut candidate = param[i] + step;

            // Strictly enforce box bounds
            if let Some(bounds) = &var.bounds {
                candidate = bounds.clamp(candidate);
            }
            next_param[i] = candidate;
        }

        Ok(next_param)
    }
}
