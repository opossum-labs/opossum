use std::sync::Arc;
use argmin::core::{CostFunction, Error};

use crate::{
    applier::ParameterApplier,
    coupling::ParameterCoupling,
    operand::{EvaluationContext, Operand},
    resolver::ParameterResolver,
    variable::FreeVariable,
};
use opossum_core::{opm_document::OpmDocument, error::OpmResult};

/// Default penalty applied for fatal simulation failures (e.g., ray tracing errors)[cite: 1].
pub const DEFAULT_FATAL_PENALTY: f64 = 1.0e12;

/// Default penalty weight scaling parameter box bound violations[cite: 1].
pub const DEFAULT_BOUNDS_PENALTY_WEIGHT: f64 = 1.0e4;

/// Central cost function evaluator $\Phi(x)$ managing blueprint cloning, parameter mapping,
/// simulation execution, and operand evaluation[cite: 1].
pub struct MeritFunction {
    blueprint: OpmDocument,
    free_variables: Vec<FreeVariable>,
    couplings: Vec<ParameterCoupling>,
    operands: Vec<Arc<dyn Operand>>,
    fatal_penalty: f64,
    bounds_penalty_weight: f64,
}

impl MeritFunction {
    /// Creates a new `MeritFunction` instance.
    ///
    /// # Errors
    /// Returns an error if the parameter and coupling configuration contains duplicates or cycles[cite: 1].
    pub fn new(
        blueprint: OpmDocument,
        free_variables: Vec<FreeVariable>,
        couplings: Vec<ParameterCoupling>,
        operands: Vec<Arc<dyn Operand>>,
    ) -> OpmResult<Self> {
        ParameterResolver::validate_configuration(&free_variables, &couplings)?;

        Ok(Self {
            blueprint,
            free_variables,
            couplings,
            operands,
            fatal_penalty: DEFAULT_FATAL_PENALTY,
            bounds_penalty_weight: DEFAULT_BOUNDS_PENALTY_WEIGHT,
        })
    }

    /// Sets a custom penalty value for fatal simulation crashes[cite: 1].
    pub fn with_fatal_penalty(mut self, penalty: f64) -> Self {
        self.fatal_penalty = penalty;
        self
    }

    /// Sets a custom penalty weight for box bound violations[cite: 1].
    pub fn with_bounds_penalty_weight(mut self, weight: f64) -> Self {
        self.bounds_penalty_weight = weight;
        self
    }

    /// Returns the registered free variables.
    pub fn free_variables(&self) -> &[FreeVariable] {
        &self.free_variables
    }

    /// Returns the registered parameter couplings[cite: 1].
    pub fn couplings(&self) -> &[ParameterCoupling] {
        &self.couplings
    }

    /// Evaluates the total merit value $\Phi(x)$ for candidate vector $x$[cite: 1].
    ///
    /// Adheres to the strict no-panic guarantee: any simulation failure or unresolvable
    /// metric is caught and converted into a large penalty cost[cite: 1].
    pub fn evaluate(&self, x: &[f64]) -> f64 {
        // 1. Calculate continuous penalty for box-bound violations[cite: 1]
        let mut total_cost = 0.0;
        for (var, &val) in self.free_variables.iter().zip(x.iter()) {
            if let Some(bounds) = &var.bounds {
                let violation = bounds.penalty(val);
                if violation > 0.0 {
                    total_cost += self.bounds_penalty_weight * violation;
                }
            }
        }

        // 2. Clone pristine blueprint document (thread-isolated)[cite: 1]
        let mut working_doc = self.blueprint.clone();

        // 3. Resolve solver parameters and couplings[cite: 1]
        let parameter_map = match ParameterResolver::resolve(&self.free_variables, &self.couplings, x) {
            Ok(map) => map,
            Err(_) => return self.fatal_penalty,
        };

        // 4. Apply all parameters to the document[cite: 1]
        for (address, value) in parameter_map {
            if ParameterApplier::set_value(&mut working_doc, &address, value).is_err() {
                return self.fatal_penalty;
            }
        }

        // 5. Rebuild surfaces across modified nodes[cite: 1]
        if ParameterApplier::finalize_update(&mut working_doc).is_err() {
            return self.fatal_penalty;
        }

        // 6. Run optical simulation across all configured analyzers[cite: 1, 2]
        if working_doc.analyze_headless().is_err() {
            return self.fatal_penalty;
        }

        // 7. Evaluate all configured operands[cite: 1]
        let context = EvaluationContext::new(&working_doc);
        for operand in &self.operands {
            match operand.evaluate(&context) {
                Ok(operand_cost) => {
                    total_cost += operand_cost;
                }
                Err(err) => {
                    eprintln!("DEBUG: Operand '{}' failed with: {err:?}", operand.name());
                    total_cost += self.fatal_penalty * 0.1;
                }
            }
        }

        total_cost
    }
}

impl CostFunction for MeritFunction {
    type Param = Vec<f64>;
    type Output = f64;

    fn cost(&self, param: &Self::Param) -> Result<Self::Output, Error> {
        Ok(self.evaluate(param))
    }
}