use argmin::core::Executor;
use argmin::solver::neldermead::NelderMead;
use std::sync::Arc;

use crate::{
    algorithm::OptimizationAlgorithm, applier::ParameterApplier, coupling::ParameterCoupling,
    merit::MeritFunction, operand::Operand, resolver::ParameterResolver,
    summary::OptimizationSummary, variable::FreeVariable,
};
use opossum_core::{
    error::{OpmResult, OpossumError},
    opm_document::OpmDocument,
};

/// High-level optical system optimizer orchestrating variables, merit criteria, and solvers[cite: 1].
pub struct OpticOptimizer {
    free_variables: Vec<FreeVariable>,
    couplings: Vec<ParameterCoupling>,
    operands: Vec<Arc<dyn Operand>>,
    algorithm: OptimizationAlgorithm,
    max_iterations: u64,
    target_cost: f64,
}

impl Default for OpticOptimizer {
    fn default() -> Self {
        Self {
            free_variables: Vec::new(),
            couplings: Vec::new(),
            operands: Vec::new(),
            algorithm: OptimizationAlgorithm::default(),
            max_iterations: 100,
            target_cost: 1e-8,
        }
    }
}

impl OpticOptimizer {
    /// Creates a new `OpticOptimizer` builder with default configuration[cite: 1].
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a free parameter to be manipulated by the solver[cite: 1].
    pub fn add_variable(mut self, variable: FreeVariable) -> Self {
        self.free_variables.push(variable);
        self
    }

    /// Registers a parameter coupling rule (e.g. Opposite, TotalLengthLock)[cite: 1].
    pub fn add_coupling(mut self, coupling: ParameterCoupling) -> Self {
        self.couplings.push(coupling);
        self
    }

    /// Registers an evaluation criterion (operand) for the merit function[cite: 1].
    pub fn add_operand(mut self, operand: impl Operand + 'static) -> Self {
        self.operands.push(Arc::new(operand));
        self
    }

    /// Selects the mathematical optimization algorithm[cite: 1].
    pub fn with_algorithm(mut self, algorithm: OptimizationAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Sets the maximum allowable iteration steps[cite: 1].
    pub fn with_max_iterations(mut self, max_iterations: u64) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Sets the target merit function cost for early stopping[cite: 1].
    pub fn with_target_cost(mut self, target_cost: f64) -> Self {
        self.target_cost = target_cost;
        self
    }

    /// Executes the optimization process against the provided `OpmDocument`[cite: 1, 2].
    ///
    /// 1. Evaluates baseline cost[cite: 1].
    /// 2. Executes numerical solver in headless mode[cite: 1, 12].
    /// 3. Writes best found parameters back into the document[cite: 1, 8].
    /// 4. Rebuilds optical surfaces and synchronizes detector states[cite: 1, 4, 8].
    pub fn optimize(&self, doc: &mut OpmDocument) -> OpmResult<OptimizationSummary> {
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

        // Validate coupling integrity upfront[cite: 1]
        ParameterResolver::validate_configuration(&self.free_variables, &self.couplings)?;

        // Build pristine MeritFunction[cite: 1]
        let merit = MeritFunction::new(
            doc.clone(),
            self.free_variables.clone(),
            self.couplings.clone(),
            self.operands.clone(),
        )?;

        // Measure initial baseline cost[cite: 1]
        let initial_vector: Vec<f64> = self
            .free_variables
            .iter()
            .map(|v| v.initial_value)
            .collect();
        let initial_cost = merit.evaluate(&initial_vector);

        // Dispatch solver according to selected algorithm[cite: 1]
        let (best_params, final_cost, iterations, converged) = match self.algorithm {
            OptimizationAlgorithm::NelderMead { sd_tolerance } => {
                let initial_simplex =
                    OptimizationAlgorithm::build_initial_simplex(&self.free_variables)?;
                let solver = NelderMead::new(initial_simplex)
                    .with_sd_tolerance(sd_tolerance)
                    .map_err(|e| OpossumError::Other(format!("Failed to set tolerance: {e}")))?;

                let res = Executor::new(merit, solver)
                    .configure(|state| {
                        state
                            .max_iters(self.max_iterations)
                            .target_cost(self.target_cost)
                    })
                    .run()
                    .map_err(|e| {
                        OpossumError::Other(format!("Optimization execution failed: {e}"))
                    })?;

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

        // Apply optimized values to active document[cite: 1, 8]
        for (address, value) in &resolved_map {
            ParameterApplier::set_value(doc, address, *value)?;
        }
        ParameterApplier::finalize_update(doc)?;

        // Execute headless simulation so detector nodes are populated with optimal state[cite: 10, 12]
        doc.analyze_headless()?;

        Ok(OptimizationSummary {
            initial_cost,
            final_cost,
            iterations,
            converged,
            optimized_parameters: resolved_map,
        })
    }
}
