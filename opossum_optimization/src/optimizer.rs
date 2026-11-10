use argmin::{
    core::{Executor, State, TerminationReason},
    solver::{
        linesearch::{BacktrackingLineSearch, condition::ArmijoCondition},
        neldermead::NelderMead,
        quasinewton::LBFGS,
    },
};
use std::sync::{Arc, atomic::AtomicBool};

use crate::{
    DampedLeastSquares, OptimizationConfig, OptimizationProgressEvent,
    algorithm::OptimizationAlgorithm, applier::ParameterApplier, asa::AdaptiveSimulatedAnnealing,
    coupling::ParameterCoupling, dls::DlsIterState, merit::MeritFunction, operand::Operand,
    progress::ProgressObserver, reader::MetricReader, resolver::ParameterResolver,
    summary::OptimizationSummary, variable::FreeVariable,
};
use opossum_core::{
    error::{OpmResult, OpossumError},
    opm_document::OpmDocument,
};

/// Internal container holding the raw numerical output of an executed solver.
struct SolverOutcome {
    best_params: Vec<f64>,
    final_cost: f64,
    iterations: u64,
    converged: bool,
}

/// High-level optical system optimizer bound to an active `OpmDocument`.
pub struct OpticOptimizer<'a> {
    doc: &'a mut OpmDocument,
    free_variables: Vec<FreeVariable>,
    couplings: Vec<ParameterCoupling>,
    operands: Vec<Arc<dyn Operand>>,
    algorithm: OptimizationAlgorithm,
    target_cost: f64,
}

impl<'a> OpticOptimizer<'a> {
    /// Creates a new `OpticOptimizer` bound to the provided optical document.
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

    /// Creates an `OpticOptimizer` pre-configured from a serialized `OptimizationConfig`.
    ///
    /// Any free variables lacking an explicit initial value have their baseline values
    /// populated directly from the current state of the document.
    ///
    /// # Errors
    /// Returns an error if the configuration contains invalid parameters.
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
    ///
    /// # Errors
    /// Returns an error if the start value of the given free variable cannot be determined.
    pub fn add_variable(mut self, mut variable: FreeVariable) -> OpmResult<Self> {
        if variable.initial_value.is_none() {
            let current_val = MetricReader::read_parameter_value(self.doc, &variable.address)?;
            variable.initial_value = Some(current_val);
        }
        self.free_variables.push(variable);
        Ok(self)
    }

    /// Registers a parameter coupling rule (e.g. `Opposite`, `TotalLengthLock`).
    #[must_use]
    pub fn add_coupling(mut self, coupling: ParameterCoupling) -> Self {
        self.couplings.push(coupling);
        self
    }

    /// Registers an evaluation criterion (operand) for the merit function.
    #[must_use]
    pub fn add_operand(mut self, operand: impl Operand + 'static) -> Self {
        self.operands.push(Arc::new(operand));
        self
    }

    /// Selects the optimization algorithm and its solver-specific settings.
    #[must_use]
    pub const fn with_algorithm(mut self, algorithm: OptimizationAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Sets the target cost threshold for early stopping.
    #[must_use]
    pub const fn with_target_cost(mut self, target_cost: f64) -> Self {
        self.target_cost = target_cost;
        self
    }

    /// Executes the optimization, updates the model in-place, and refreshes detector states.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// - no free variables were specified for the optimization
    /// - no operands were specified for the merit function
    /// - the free variable or operands definitions are invalid
    /// - the optimization solver encounters a fatal error.
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

        ParameterResolver::validate_configuration(&self.free_variables, &self.couplings)?;

        let merit = MeritFunction::new(
            self.doc.clone(),
            self.free_variables.clone(),
            self.couplings.clone(),
            self.operands.clone(),
        )?;

        let initial_vector = self.collect_initial_vector()?;
        let initial_cost = merit.evaluate(&initial_vector);

        let outcome = self.execute_solver(merit, initial_vector)?;

        let resolved_map = ParameterResolver::resolve(
            &self.free_variables,
            &self.couplings,
            &outcome.best_params,
        )?;

        for (address, value) in &resolved_map {
            ParameterApplier::set_value(self.doc, address, *value)?;
        }
        ParameterApplier::finalize_update(self.doc)?;
        self.doc.analyze_headless()?;

        Ok(OptimizationSummary {
            initial_cost,
            final_cost: outcome.final_cost,
            iterations: outcome.iterations,
            converged: outcome.converged,
            optimized_parameters: resolved_map,
        })
    }

    /// Executes the optimization, updates the model in-place, and refreshes detector states.
    ///
    /// This function is similar to [`OpticOptimizer::optimize`] but allows for adding an observer function and an
    /// abort functionality.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// - no free variables were specified for the optimization
    /// - no operands were specified for the merit function
    /// - the free variable or operands definitions are invalid
    /// - the optimization solver encounters a fatal error.
    pub fn optimize_with_observer<F>(
        self,
        cancel_flag: Arc<AtomicBool>,
        mut progress_callback: F,
    ) -> OpmResult<OptimizationSummary>
    where
        F: FnMut(OptimizationProgressEvent) -> bool + Send + 'static,
    {
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

        ParameterResolver::validate_configuration(&self.free_variables, &self.couplings)?;

        let merit = MeritFunction::new(
            self.doc.clone(),
            self.free_variables.clone(),
            self.couplings.clone(),
            self.operands.clone(),
        )?;

        let initial_vector = self.collect_initial_vector()?;
        let initial_cost = merit.evaluate(&initial_vector);

        let max_iters = match self.algorithm {
            OptimizationAlgorithm::NelderMead { max_iterations, .. }
            | OptimizationAlgorithm::LBFGS { max_iterations, .. }
            | OptimizationAlgorithm::SimulatedAnnealing { max_iterations, .. }
            | OptimizationAlgorithm::Dls { max_iterations, .. } => max_iterations,
        };

        progress_callback(OptimizationProgressEvent::Started {
            initial_cost,
            total_max_iterations: max_iters,
        });

        let best_tracker = Arc::new(std::sync::Mutex::new(Some((
            initial_vector.clone(),
            initial_cost,
            0,
        ))));

        let observer = ProgressObserver::new(
            progress_callback,
            cancel_flag,
            self.free_variables.clone(),
            self.couplings.clone(),
            best_tracker.clone(),
        );

        let outcome =
            self.execute_solver_with_observer(merit, &initial_vector, observer, &best_tracker)?;

        // Parameter auflösen und in das Dokument schreiben
        let resolved_map = ParameterResolver::resolve(
            &self.free_variables,
            &self.couplings,
            &outcome.best_params,
        )?;

        for (address, value) in &resolved_map {
            ParameterApplier::set_value(self.doc, address, *value)?;
        }
        ParameterApplier::finalize_update(self.doc)?;
        self.doc.analyze_headless()?;

        Ok(OptimizationSummary {
            initial_cost,
            final_cost: outcome.final_cost,
            iterations: outcome.iterations,
            converged: outcome.converged,
            optimized_parameters: resolved_map,
        })
    }

    fn execute_solver_with_observer(
        &self,
        merit: MeritFunction,
        initial_vector: &[f64],
        observer: ProgressObserver,
        best_tracker: &Arc<std::sync::Mutex<Option<(Vec<f64>, f64, u64)>>>,
    ) -> OpmResult<SolverOutcome> {
        let run_result = match self.algorithm {
            OptimizationAlgorithm::NelderMead {
                max_iterations,
                sd_tolerance,
            } => {
                let initial_simplex =
                    OptimizationAlgorithm::build_initial_simplex(&self.free_variables)?;
                let solver = NelderMead::new(initial_simplex)
                    .with_sd_tolerance(sd_tolerance)
                    .map_err(|e| OpossumError::Other(format!("Failed to set tolerance: {e}")))?;
                Executor::new(merit, solver)
                    .configure(|state| {
                        state
                            .max_iters(max_iterations)
                            .target_cost(self.target_cost)
                    })
                    .add_observer(observer, argmin::core::observers::ObserverMode::Always)
                    .run()
                    .map(|res| self.handle_solver_state(res.state()))
            }
            OptimizationAlgorithm::LBFGS {
                max_iterations,
                gradient_tolerance,
                history_size,
                gradient_method,
            } => {
                let merit = merit.with_gradient_method(gradient_method);

                // Armijo condition with relaxed slope c1 = 1e-4
                let condition = ArmijoCondition::new(1e-4).map_err(|e| {
                    OpossumError::Other(format!("Failed to create Armijo condition: {e}"))
                })?;

                // Robust geometric backtracking line search (contraction rho = 0.8)
                let linesearch = BacktrackingLineSearch::new(condition)
                    .rho(0.8)
                    .map_err(|e| {
                        OpossumError::Other(format!("Failed to set backtracking line search: {e}"))
                    })?;

                let solver = LBFGS::new(linesearch, history_size)
                    .with_tolerance_grad(gradient_tolerance)
                    .map_err(|e| OpossumError::Other(format!("Failed to set tolerance: {e}")))?;

                Executor::new(merit, solver)
                    .configure(|state| {
                        state
                            .param(initial_vector.to_vec())
                            .max_iters(max_iterations)
                            .target_cost(self.target_cost)
                    })
                    .add_observer(observer, argmin::core::observers::ObserverMode::Always)
                    .run()
                    .map(|res| self.handle_solver_state(res.state()))
            }
            OptimizationAlgorithm::SimulatedAnnealing {
                max_iterations,
                initial_temperature,
                cooling_rate,
                ..
            } => {
                let solver =
                    AdaptiveSimulatedAnnealing::new(&merit, initial_temperature, cooling_rate)
                        .with_adaptation_cycle(20);
                Executor::new(merit, solver)
                    .configure(|state| {
                        state
                            .param(initial_vector.to_vec())
                            .max_iters(max_iterations)
                            .target_cost(self.target_cost)
                    })
                    .add_observer(observer, argmin::core::observers::ObserverMode::Always)
                    .run()
                    .map(|res| self.handle_solver_state(res.state()))
            }
            OptimizationAlgorithm::Dls {
                max_iterations,
                initial_damping,
                gradient_tolerance,
            } => {
                let solver = DampedLeastSquares::new(merit.clone())
                    .with_initial_damping(initial_damping)
                    .with_gradient_tolerance(gradient_tolerance);

                Executor::new(merit, solver)
                    .configure(|state: DlsIterState| {
                        state
                            .param(initial_vector.to_vec())
                            .max_iters(max_iterations)
                            .target_cost(self.target_cost)
                    })
                    .add_observer(observer, argmin::core::observers::ObserverMode::Always)
                    .run()
                    .map(|res| self.handle_solver_state(res.state()))
            }
        };

        match run_result {
            Ok(inner_res) => inner_res,
            Err(e) => {
                // Bei Nutzerabbruch: Bisher besten Zustand aus dem Tracker retten
                if e.to_string().contains("cancelled by user")
                    && let Ok(guard) = best_tracker.lock()
                    && let Some((best_params, final_cost, iterations)) = guard.clone()
                {
                    return Ok(SolverOutcome {
                        best_params,
                        final_cost,
                        iterations,
                        converged: false,
                    });
                }
                Err(OpossumError::Other(format!("Optimization failed: {e}")))
            }
        }
    }
    fn handle_solver_state<I>(&self, state: &I) -> OpmResult<SolverOutcome>
    where
        I: State<Param = Vec<f64>, Float = f64>,
    {
        let best_params = state
            .get_best_param()
            .cloned()
            .or_else(|| state.get_param().cloned())
            .ok_or_else(|| OpossumError::Other("Solver produced no solution".to_string()))?;

        let converged = match state.get_termination_reason() {
            Some(TerminationReason::TargetCostReached | TerminationReason::SolverConverged) => true,
            _ => state.get_best_cost() <= self.target_cost,
        };

        Ok(SolverOutcome {
            best_params,
            final_cost: state.get_best_cost(),
            iterations: state.get_iter(),
            converged,
        })
    }
    /// Safely collects the initial parameter vector from all free variables.
    fn collect_initial_vector(&self) -> OpmResult<Vec<f64>> {
        self.free_variables
            .iter()
            .map(|v| {
                v.initial_value.ok_or_else(|| {
                    OpossumError::Other(format!(
                        "FreeVariable '{}' has uninitialized start value",
                        v.name
                    ))
                })
            })
            .collect()
    }

    /// Dispatches execution to the configured numerical solver.
    fn execute_solver(
        &self,
        merit: MeritFunction,
        initial_vector: Vec<f64>,
    ) -> OpmResult<SolverOutcome> {
        match self.algorithm {
            OptimizationAlgorithm::NelderMead {
                max_iterations,
                sd_tolerance,
            } => self.execute_nelder_mead(merit, max_iterations, sd_tolerance),
            OptimizationAlgorithm::LBFGS {
                max_iterations,
                gradient_tolerance,
                history_size,
                gradient_method,
            } => {
                let merit = merit.with_gradient_method(gradient_method);

                let condition = ArmijoCondition::new(1e-4).map_err(|e| {
                    OpossumError::Other(format!("Failed to create Armijo condition: {e}"))
                })?;

                let linesearch = BacktrackingLineSearch::new(condition)
                    .rho(0.8)
                    .map_err(|e| {
                        OpossumError::Other(format!("Failed to set backtracking line search: {e}"))
                    })?;

                let solver = LBFGS::new(linesearch, history_size)
                    .with_tolerance_grad(gradient_tolerance)
                    .map_err(|e| OpossumError::Other(format!("Failed to set tolerance: {e}")))?;

                let res = Executor::new(merit, solver)
                    .configure(|state| {
                        state
                            .param(initial_vector.clone())
                            .max_iters(max_iterations)
                            .target_cost(self.target_cost)
                    })
                    .run()
                    .map_err(|e| OpossumError::Other(format!("Optimization failed: {e}")))?;

                self.handle_solver_state(res.state())
            }
            OptimizationAlgorithm::SimulatedAnnealing {
                max_iterations,
                initial_temperature,
                cooling_rate,
                ..
            } => self.execute_simulated_annealing(
                merit,
                initial_vector,
                max_iterations,
                initial_temperature,
                cooling_rate,
            ),
            OptimizationAlgorithm::Dls {
                max_iterations,
                initial_damping,
                gradient_tolerance,
            } => {
                let solver = DampedLeastSquares::new(merit.clone())
                    .with_initial_damping(initial_damping)
                    .with_gradient_tolerance(gradient_tolerance);

                let res = Executor::new(merit, solver)
                    .configure(|state: DlsIterState| {
                        state
                            .param(initial_vector.clone())
                            .max_iters(max_iterations)
                            .target_cost(self.target_cost)
                    })
                    .run()
                    .map_err(|e| OpossumError::Other(format!("Optimization failed: {e}")))?;

                self.handle_solver_state(res.state())
            }
        }
    }

    fn execute_nelder_mead(
        &self,
        merit: MeritFunction,
        max_iterations: u64,
        sd_tolerance: f64,
    ) -> OpmResult<SolverOutcome> {
        let initial_simplex = OptimizationAlgorithm::build_initial_simplex(&self.free_variables)?;
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
            .map_err(|e| OpossumError::Other(format!("Nelder-Mead failed: {e}")))?;

        let state = res.state();
        let best_params = state
            .best_param
            .clone()
            .ok_or_else(|| OpossumError::Other("Solver produced no solution".to_string()))?;

        let converged = match state.get_termination_reason() {
            Some(TerminationReason::TargetCostReached | TerminationReason::SolverConverged) => true,
            _ => state.best_cost <= self.target_cost,
        };

        Ok(SolverOutcome {
            best_params,
            final_cost: state.best_cost,
            iterations: state.iter,
            converged,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_simulated_annealing(
        &self,
        merit: MeritFunction,
        initial_vector: Vec<f64>,
        max_iterations: u64,
        initial_temperature: f64,
        cooling_rate: f64,
    ) -> OpmResult<SolverOutcome> {
        let solver = AdaptiveSimulatedAnnealing::new(&merit, initial_temperature, cooling_rate)
            .with_adaptation_cycle(20);

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
        let best_params = state.best_param.clone().ok_or_else(|| {
            OpossumError::Other("Simulated Annealing produced no solution".to_string())
        })?;

        let converged = match state.get_termination_reason() {
            Some(TerminationReason::TargetCostReached | TerminationReason::SolverConverged) => true,
            _ => state.best_cost <= self.target_cost,
        };

        Ok(SolverOutcome {
            best_params,
            final_cost: state.best_cost,
            iterations: state.iter,
            converged,
        })
    }
}
