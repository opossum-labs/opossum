use argmin::{
    core::{Executor, State, TerminationReason},
    solver::neldermead::NelderMead,
};
use std::sync::{Arc, atomic::AtomicBool};

use crate::{
    DampedLeastSquares, applier::ParameterApplier, asa::AdaptiveSimulatedAnnealing,
    dls::DlsIterState, merit::MeritFunction, operand::Operand, progress::ProgressObserver,
    reader::MetricReader, resolver::ParameterResolver, summary::OptimizationSummary,
};
use opossum_core::{
    error::{OpmResult, OpossumError},
    opm_document::{
        OpmDocument, OptimizationConfig,
        optimization::{
            FreeVariable, OptimizationAlgorithm, OptimizationProgressEvent, ParameterCoupling,
        },
    },
};

/// Internal merit cost threshold for early solver termination.
const DEFAULT_TARGET_COST: f64 = 1e-8;

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
            target_cost: DEFAULT_TARGET_COST,
        }
    }

    /// Creates an `OpticOptimizer` pre-configured from a serialized `OptimizationConfig`.
    ///
    /// # Errors
    /// Returns an error if the configuration contains invalid parameter addresses.
    pub fn from_config(doc: &'a mut OpmDocument, config: OptimizationConfig) -> OpmResult<Self> {
        let mut optimizer = Self::new(doc).with_algorithm(config.algorithm);

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

    /// Registers a free parameter and validates that its address exists in the current document.
    ///
    /// # Errors
    /// Returns an error if the parameter address cannot be resolved in the document.
    pub fn add_variable(mut self, variable: FreeVariable) -> OpmResult<Self> {
        let _ = MetricReader::read_parameter_value(self.doc, &variable.address)?;
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

        progress_callback(OptimizationProgressEvent::Started { initial_cost });

        let best_tracker = Arc::new(std::sync::Mutex::new(Some((
            initial_vector.clone(),
            initial_cost,
            0,
        ))));

        let observer = ProgressObserver::new(progress_callback, cancel_flag, best_tracker.clone());

        let outcome =
            self.execute_solver_with_observer(merit, &initial_vector, observer, &best_tracker)?;

        // Resolve parameters and write them into the document
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

    /// Dispatches execution with a progress observer to the configured numerical solver.
    fn execute_solver_with_observer(
        &self,
        merit: MeritFunction,
        initial_vector: &[f64],
        observer: ProgressObserver,
        best_tracker: &crate::progress::BestStateTracker,
    ) -> OpmResult<SolverOutcome> {
        let run_result = match self.algorithm {
            OptimizationAlgorithm::NelderMead {
                max_iterations,
                sd_tolerance,
            } => self.run_nelder_mead_observed(
                merit,
                initial_vector,
                max_iterations,
                sd_tolerance,
                observer,
            ),
            OptimizationAlgorithm::AdaptiveSimulatedAnnealing {
                max_iterations,
                initial_temperature,
                cooling_rate,
                adaptation_cycle,
            } => self.run_asa_observed(
                merit,
                initial_vector,
                max_iterations,
                initial_temperature,
                cooling_rate,
                adaptation_cycle,
                observer,
            ),
            OptimizationAlgorithm::Dls {
                max_iterations,
                initial_damping,
                gradient_tolerance,
            } => self.run_dls_observed(
                merit,
                initial_vector,
                max_iterations,
                initial_damping,
                gradient_tolerance,
                observer,
            ),
        };

        match run_result {
            Ok(inner_res) => inner_res,
            Err(e) => {
                // Recover the best state so far from the tracker if cancelled by the user
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

    /// Configures and executes Nelder-Mead with a progress observer.
    fn run_nelder_mead_observed(
        &self,
        merit: MeritFunction,
        initial_vector: &[f64],
        max_iterations: u64,
        sd_tolerance: f64,
        observer: ProgressObserver,
    ) -> Result<OpmResult<SolverOutcome>, argmin::core::Error> {
        let initial_simplex =
            OptimizationAlgorithm::build_initial_simplex(&self.free_variables, initial_vector)
                .map_err(|e| argmin::core::Error::msg(e.to_string()))?;
        let solver = NelderMead::new(initial_simplex).with_sd_tolerance(sd_tolerance)?;

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

    /// Configures and executes Adaptive Simulated Annealing with a progress observer.
    #[allow(clippy::too_many_arguments)]
    fn run_asa_observed(
        &self,
        merit: MeritFunction,
        initial_vector: &[f64],
        max_iterations: u64,
        initial_temperature: f64,
        cooling_rate: f64,
        adaptation_cycle: u64,
        observer: ProgressObserver,
    ) -> Result<OpmResult<SolverOutcome>, argmin::core::Error> {
        let solver = AdaptiveSimulatedAnnealing::new(&merit, initial_temperature, cooling_rate)
            .with_adaptation_cycle(adaptation_cycle);

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

    /// Configures and executes Damped Least Squares (DLS) with a progress observer.
    #[allow(clippy::too_many_arguments)]
    fn run_dls_observed(
        &self,
        merit: MeritFunction,
        initial_vector: &[f64],
        max_iterations: u64,
        initial_damping: f64,
        gradient_tolerance: f64,
        observer: ProgressObserver,
    ) -> Result<OpmResult<SolverOutcome>, argmin::core::Error> {
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

    /// Collects the current parameter values directly from the active [`OpmDocument`].
    fn collect_initial_vector(&self) -> OpmResult<Vec<f64>> {
        self.free_variables
            .iter()
            .map(|v| MetricReader::read_parameter_value(self.doc, &v.address))
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
            } => self.execute_nelder_mead(merit, &initial_vector, max_iterations, sd_tolerance),
            OptimizationAlgorithm::AdaptiveSimulatedAnnealing {
                max_iterations,
                initial_temperature,
                cooling_rate,
                adaptation_cycle,
            } => self.execute_adaptive_simulated_annealing(
                merit,
                initial_vector,
                max_iterations,
                initial_temperature,
                cooling_rate,
                adaptation_cycle,
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
        initial_vector: &[f64],
        max_iterations: u64,
        sd_tolerance: f64,
    ) -> OpmResult<SolverOutcome> {
        let initial_simplex =
            OptimizationAlgorithm::build_initial_simplex(&self.free_variables, initial_vector)?;
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
    fn execute_adaptive_simulated_annealing(
        &self,
        merit: MeritFunction,
        initial_vector: Vec<f64>,
        max_iterations: u64,
        initial_temperature: f64,
        cooling_rate: f64,
        adaptation_cycle: u64,
    ) -> OpmResult<SolverOutcome> {
        let solver = AdaptiveSimulatedAnnealing::new(&merit, initial_temperature, cooling_rate)
            .with_adaptation_cycle(adaptation_cycle);

        let res = Executor::new(merit, solver)
            .configure(|state| {
                state
                    .param(initial_vector)
                    .max_iters(max_iterations)
                    .target_cost(self.target_cost)
            })
            .run()
            .map_err(|e| {
                OpossumError::Other(format!("Adaptive Simulated Annealing failed: {e}"))
            })?;

        let state = res.state();
        let best_params = state.best_param.clone().ok_or_else(|| {
            OpossumError::Other("Adaptive Simulated Annealing produced no solution".to_string())
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
