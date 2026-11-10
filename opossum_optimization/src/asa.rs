use crate::{merit::MeritFunction, variable::FreeVariable};
use argmin::core::{CostFunction, Error, IterState, KV, Problem, Solver, State};
use rand::RngExt;

pub type AsaIterState = IterState<Vec<f64>, (), (), (), (), f64>;

/// Adaptive Simulated Annealing solver featuring per-parameter step size tuning
/// based on local acceptance ratios (1/5th success rule / Corana adaptation).
pub struct AdaptiveSimulatedAnnealing {
    free_vars: Vec<FreeVariable>,
    initial_temperature: f64,
    temperature: f64,
    cooling_rate: f64,
    adaptation_cycle: u64,
    adaptation_rate: f64,

    // Per-variable step sizes and diagnostic counters
    step_sizes: Vec<f64>,
    proposed_counts: Vec<u64>,
    accepted_counts: Vec<u64>,
}

impl AdaptiveSimulatedAnnealing {
    /// Creates a new Adaptive Simulated Annealing solver.
    #[must_use]
    pub fn new(merit: &MeritFunction, initial_temperature: f64, cooling_rate: f64) -> Self {
        let vars = merit.free_variables().to_vec();
        let n = vars.len();

        // Initialize step sizes to 10% of parameter span or fallback
        let step_sizes = vars
            .iter()
            .map(|v| v.bounds.as_ref().map_or(0.01, |b| (b.max - b.min) * 0.10))
            .collect();

        Self {
            free_vars: vars,
            initial_temperature,
            temperature: initial_temperature,
            cooling_rate,
            adaptation_cycle: 20, // Re-tune step sizes every 20 evaluations per variable
            adaptation_rate: 2.0, // Scale factor for step expansion/contraction
            step_sizes,
            proposed_counts: vec![0; n],
            accepted_counts: vec![0; n],
        }
    }

    /// Sets the adaptation cycle length (number of attempts per parameter before adjusting step size).
    #[must_use]
    pub const fn with_adaptation_cycle(mut self, cycle: u64) -> Self {
        self.adaptation_cycle = cycle;
        self
    }

    /// Adjusts step sizes based on recent acceptance history.
    fn adapt_step_sizes(&mut self) {
        for i in 0..self.free_vars.len() {
            if self.proposed_counts[i] == 0 {
                continue;
            }

            let ratio = self.accepted_counts[i] as f64 / self.proposed_counts[i] as f64;
            let bounds_span = self.free_vars[i]
                .bounds
                .as_ref()
                .map_or(1.0, |b| b.max - b.min);

            if ratio > 0.40 {
                // Too many accepted steps: increase step size to explore faster
                let factor = 1.0 + self.adaptation_rate * (ratio - 0.40) / 0.60;
                self.step_sizes[i] *= factor;
            } else if ratio < 0.20 {
                // Too many rejected steps: decrease step size to fit valley curvature
                let factor = 1.0 + self.adaptation_rate * (0.20 - ratio) / 0.20;
                self.step_sizes[i] /= factor;
            }

            // Clamp step size to sensible physical ranges
            let min_step = 1e-6;
            let max_step = bounds_span * 0.50;
            self.step_sizes[i] = self.step_sizes[i].clamp(min_step, max_step);

            // Reset epoch counters
            self.proposed_counts[i] = 0;
            self.accepted_counts[i] = 0;
        }
    }
}

impl<O> Solver<O, AsaIterState> for AdaptiveSimulatedAnnealing
where
    O: CostFunction<Param = Vec<f64>, Output = f64>,
{
    fn name(&self) -> &str {
        "Adaptive Simulated Annealing (Corana 1/5 Rule)"
    }

    fn init(
        &mut self,
        problem: &mut Problem<O>,
        state: AsaIterState,
    ) -> Result<(AsaIterState, Option<KV>), Error> {
        let param = state
            .get_param()
            .cloned()
            .ok_or_else(|| argmin::core::Error::msg("ASA requires an initial parameter vector"))?;
        let cost0 = problem.cost(&param)?;
        self.temperature = self.initial_temperature;

        Ok((state.param(param).cost(cost0), None))
    }

    fn next_iter(
        &mut self,
        problem: &mut Problem<O>,
        state: AsaIterState,
    ) -> Result<(AsaIterState, Option<KV>), Error> {
        let current_param = state.get_param().cloned().unwrap();
        let current_cost = state.get_cost();
        let n = self.free_vars.len();

        let mut rng = rand::rng();

        // 1. Randomly pick exactly one parameter to perturb
        let idx = rng.random_range(0..n);
        let mut candidate_param = current_param.clone();

        // 2. Sample perturbation using current adaptive step size
        let delta = rng.random_range(-1.0..=1.0) * self.step_sizes[idx];
        let mut new_val = current_param[idx] + delta;

        // Respect parameter bounds
        if let Some(bounds) = &self.free_vars[idx].bounds {
            new_val = bounds.clamp(new_val);
        }
        candidate_param[idx] = new_val;

        // 3. Evaluate new cost
        let candidate_cost = problem.cost(&candidate_param)?;
        self.proposed_counts[idx] += 1;

        // 4. Metropolis-Hastings acceptance criterion
        let delta_cost = candidate_cost - current_cost;
        let accept = if delta_cost < 0.0 {
            true
        } else {
            let p_accept = (-delta_cost / self.temperature).exp();
            rng.random_range(0.0..1.0) < p_accept
        };

        let next_state = if accept {
            self.accepted_counts[idx] += 1;
            state.param(candidate_param).cost(candidate_cost)
        } else {
            state
        };

        // 5. Periodic step size adaptation
        let total_proposals: u64 = self.proposed_counts.iter().sum();
        if total_proposals >= (self.adaptation_cycle * n as u64) {
            self.adapt_step_sizes();
        }

        // 6. Geometric temperature cooling schedule
        self.temperature *= self.cooling_rate;

        Ok((next_state, None))
    }
}
