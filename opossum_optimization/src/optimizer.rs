use argmin::core::Executor;
use argmin::solver::neldermead::NelderMead;
use std::sync::Arc;

use crate::{
    algorithm::OptimizationAlgorithm, applier::ParameterApplier, coupling::ParameterCoupling,
    merit::MeritFunction, operand::Operand, reader::MetricReader, resolver::ParameterResolver,
    summary::OptimizationSummary, variable::FreeVariable,
};
use opossum_core::{
    error::{OpmResult, OpossumError},
    opm_document::OpmDocument,
};

/// High-level optical system optimizer bound to an active `OpmDocument`
pub struct OpticOptimizer<'a> {
    doc: &'a mut OpmDocument,
    free_variables: Vec<FreeVariable>,
    couplings: Vec<ParameterCoupling>,
    operands: Vec<Arc<dyn Operand>>,
    algorithm: OptimizationAlgorithm,
    target_cost: f64,
}

impl<'a> OpticOptimizer<'a> {
    /// Creates a new `OpticOptimizer` bound to the provided optical document[cite: 1, 2].
    pub fn new(doc: &'a mut OpmDocument) -> Self {
        Self {
            doc,
            free_variables: Vec::new(),
            couplings: Vec::new(),
            operands: Vec::new(),
            algorithm: OptimizationAlgorithm::default(),
            target_cost: 1e-8,
        }
    }

    /// Registers a free parameter. If no initial value was set on the variable,
    /// it is automatically read from the current state of the document[cite: 1].
    pub fn add_variable(mut self, mut variable: FreeVariable) -> OpmResult<Self> {
        if variable.initial_value.is_none() {
            let current_val = MetricReader::read_parameter_value(self.doc, &variable.address)?;
            variable.initial_value = Some(current_val);
        }
        self.free_variables.push(variable);
        Ok(self)
    }

    /// Registers a parameter coupling rule (e.g. `Opposite`, `TotalLengthLock`)
    pub fn add_coupling(mut self, coupling: ParameterCoupling) -> Self {
        self.couplings.push(coupling);
        self
    }

    /// Registers an evaluation criterion (operand) for the merit function
    pub fn add_operand(mut self, operand: impl Operand + 'static) -> Self {
        self.operands.push(Arc::new(operand));
        self
    }

    /// Selects the optimization algorithm and its solver-specific settings[cite: 1].
    pub const fn with_algorithm(mut self, algorithm: OptimizationAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Sets the target cost threshold for early stopping[cite: 1].
    pub const fn with_target_cost(mut self, target_cost: f64) -> Self {
        self.target_cost = target_cost;
        self
    }

    /// Executes the optimization, updates the model in-place, and refreshes detector states[cite: 1, 8, 10].
    pub fn optimize(self) -> OpmResult<OptimizationSummary> {
        if self.free_variables.is_empty() {
            return Err(OpossumError::Other(
                "No free variables specified for optimization".to_string(),
            ));
        }
        if self.operands.is_empty() {
            return Err(OpossumError::Other(
                "No operands specified for merit function".to_string(),
            ));
        }

        // Validate coupling integrity[cite: 1]
        ParameterResolver::validate_configuration(&self.free_variables, &self.couplings)?;

        // Build pristine MeritFunction from blueprint clone[cite: 1]
        let merit = MeritFunction::new(
            self.doc.clone(),
            self.free_variables.clone(),
            self.couplings.clone(),
            self.operands.clone(),
        )?;

        // Measure initial baseline cost[cite: 1]
        let initial_vector: Vec<f64> = self
            .free_variables
            .iter()
            .map(|v| v.initial_value.unwrap())
            .collect();
        let initial_cost = merit.evaluate(&initial_vector);

        // Execute solver according to chosen algorithm[cite: 1]
        let (best_params, final_cost, iterations, converged) = match self.algorithm {
            OptimizationAlgorithm::NelderMead {
                max_iterations,
                sd_tolerance,
            } => {
                let initial_simplex =
                    OptimizationAlgorithm::build_initial_simplex(&self.free_variables)?;
                let solver = NelderMead::new(initial_simplex)
                    .with_sd_tolerance(sd_tolerance)
                    .map_err(|e| OpossumError::Other(format!("Failed to set tolerance: {e}")))?;

                let res = Executor::new(merit, solver)
                    .configure(|state| {
                        state
                            .max_iters(max_iterations)
                            .target_cost(self.target_cost)
                    })
                    .run()
                    .map_err(|e| OpossumError::Other(format!("Optimization failed: {e}")))?;

                let state = res.state();
                let best_p = state.best_param.clone().ok_or_else(|| {
                    OpossumError::Other("Solver produced no solution".to_string())
                })?;
                let cost = state.best_cost;
                let iters = state.iter;
                let reached_target = cost <= self.target_cost;

                (best_p, cost, iters, reached_target)
            }
        };

        // Resolve complete parameter map (including couplings)[cite: 1]
        let resolved_map =
            ParameterResolver::resolve(&self.free_variables, &self.couplings, &best_params)?;

        // Apply optimal values directly into the active document[cite: 1, 8]
        for (address, value) in &resolved_map {
            ParameterApplier::set_value(self.doc, address, *value)?;
        }
        ParameterApplier::finalize_update(self.doc)?;

        // Run headless simulation so detectors immediately hold the optimal results[cite: 10, 12]
        self.doc.analyze_headless()?;

        Ok(OptimizationSummary {
            initial_cost,
            final_cost,
            iterations,
            converged,
            optimized_parameters: resolved_map,
        })
    }
}
