use argmin::solver::neldermead::NelderMead;
use argmin::solver::quasinewton::LBFGS;
use argmin::solver::simulatedannealing::{SATempFunc, SimulatedAnnealing};
use argmin::{core::Executor, solver::linesearch::MoreThuenteLineSearch};
use std::sync::Arc;

use crate::{
    OptimizationConfig, algorithm::OptimizationAlgorithm, applier::ParameterApplier,
    coupling::ParameterCoupling, merit::MeritFunction, operand::Operand, reader::MetricReader,
    resolver::ParameterResolver, summary::OptimizationSummary, variable::FreeVariable,
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
    /// Creates a new `OpticOptimizer` bound to the provided optical document
    #[must_use]
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

    /// Creates an `OpticOptimizer` pre-configured from a serialized `OptimizationConfig`[cite: 1].
    ///
    /// Any free variables lacking an explicit initial value have their baseline values
    /// populated directly from the current state of the document[cite: 1].
    pub fn from_config(doc: &'a mut OpmDocument, config: OptimizationConfig) -> OpmResult<Self> {
        let mut optimizer = Self::new(doc)
            .with_algorithm(config.algorithm)
            .with_target_cost(config.target_cost);

        for var in config.free_variables {
            optimizer = optimizer.add_variable(var)?;
        }

        for coupling in config.couplings {
            optimizer = optimizer.add_coupling(coupling);
        }

        for operand in config.operands {
            optimizer = optimizer.add_operand(operand);
        }

        Ok(optimizer)
    }

    /// Registers a free parameter. If no initial value was set on the variable,
    /// it is automatically read from the current state of the document.
    pub fn add_variable(mut self, mut variable: FreeVariable) -> OpmResult<Self> {
        if variable.initial_value.is_none() {
            let current_val = MetricReader::read_parameter_value(self.doc, &variable.address)?;
            variable.initial_value = Some(current_val);
        }
        self.free_variables.push(variable);
        Ok(self)
    }

    /// Registers a parameter coupling rule (e.g. `Opposite`, `TotalLengthLock`)
    #[must_use]
    pub fn add_coupling(mut self, coupling: ParameterCoupling) -> Self {
        self.couplings.push(coupling);
        self
    }

    /// Registers an evaluation criterion (operand) for the merit function
    #[must_use]
    pub fn add_operand(mut self, operand: impl Operand + 'static) -> Self {
        self.operands.push(Arc::new(operand));
        self
    }

    /// Selects the optimization algorithm and its solver-specific settings
    #[must_use]
    pub const fn with_algorithm(mut self, algorithm: OptimizationAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Sets the target cost threshold for early stopping
    #[must_use]
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

        // Extract initial vector safely (no unwrap)
        let initial_vector: Vec<f64> = self
            .free_variables
            .iter()
            .map(|v| {
                v.initial_value.ok_or_else(|| {
                    OpossumError::Other(format!(
                        "FreeVariable '{}' has uninitialized start value",
                        v.name
                    ))
                })
            })
            .collect::<OpmResult<Vec<f64>>>()?;

        let initial_cost = merit.evaluate(&initial_vector);
        // Execute solver according to chosen algorithm
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
            OptimizationAlgorithm::LBFGS {
                max_iterations,
                gradient_tolerance,
                history_size,
                gradient_method,
            } => {
                let merit = merit.with_gradient_method(gradient_method);
                let linesearch = MoreThuenteLineSearch::new()
                    .with_c(1e-4, 0.9)
                    .map_err(|e| {
                        OpossumError::Other(format!("Failed to set line search parameter: {e}"))
                    })?;
                let solver = LBFGS::new(linesearch, history_size)
                    .with_tolerance_grad(gradient_tolerance)
                    .map_err(|e| OpossumError::Other(format!("Failed to set tolerance: {e}")))?;

                let res = Executor::new(merit, solver)
                    .configure(|state| {
                        state
                            .param(initial_vector)
                            .max_iters(max_iterations)
                            .target_cost(self.target_cost)
                    })
                    .run()
                    .map_err(|e| OpossumError::Other(format!("L-BFGS optimization failed: {e}")))?;

                let state = res.state();
                let best_p = state.best_param.clone().ok_or_else(|| {
                    OpossumError::Other("L-BFGS produced no solution".to_string())
                })?;
                let cost = state.best_cost;
                let iters = state.iter;
                let reached_target = cost <= self.target_cost;
                (best_p, cost, iters, reached_target)
            }
            OptimizationAlgorithm::SimulatedAnnealing {
                max_iterations,
                initial_temperature,
                stall_best,
                stall_accepted,
                reannealing_fixed,
            } => {
                // Initialize simulated annealing with temperature and cooling schedule
                let mut solver = SimulatedAnnealing::new(initial_temperature)
                    .map_err(|e| {
                        OpossumError::Other(format!("Failed to init Simulated Annealing: {e}"))
                    })?
                    .with_temp_func(SATempFunc::Exponential(0.95))
                    .with_stall_best(stall_best)
                    .with_stall_accepted(stall_accepted);

                if let Some(reanneal_steps) = reannealing_fixed {
                    solver = solver.with_reannealing_fixed(reanneal_steps);
                }

                let res = Executor::new(merit, solver)
                    .configure(|state| {
                        state
                            .param(initial_vector)
                            .max_iters(max_iterations)
                            .target_cost(self.target_cost)
                    })
                    .run()
                    .map_err(|e| OpossumError::Other(format!("Simulated Annealing failed: {e}")))?;

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
