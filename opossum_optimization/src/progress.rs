use argmin::core::{Error, KV, State, observers::Observe};
use opossum_core::error::OpossumError;
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex, atomic::{AtomicBool, Ordering},
};

use crate::{
    address::ParameterAddress, coupling::ParameterCoupling, resolver::ParameterResolver,
    variable::FreeVariable,
};

pub type BestStateTracker = Arc<Mutex<Option<(Vec<f64>, f64, u64)>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum OptimizationProgressEvent {
    Started {
        initial_cost: f64,
        total_max_iterations: u64,
    },
    Iteration {
        iteration: u64,
        cost: f64,
        best_cost: f64,
        current_params: Vec<(ParameterAddress, f64)>, // <-- Vec statt HashMap
    },
    Finished {
        final_cost: f64,
        iterations: u64,
        converged: bool,
        optimized_parameters: Vec<(ParameterAddress, f64)>, // <-- Vec statt HashMap
    },
    Aborted {
        last_cost: f64,
        iterations: u64,
        best_parameters: Vec<(ParameterAddress, f64)>, // <-- Vec statt HashMap
    },
    Failed {
        error: String,
    },
}

pub struct ProgressObserver {
    callback: Box<dyn FnMut(OptimizationProgressEvent) -> bool + Send>,
    cancel_flag: Arc<AtomicBool>,
    free_vars: Vec<FreeVariable>,
    couplings: Vec<ParameterCoupling>,
    best_tracker: BestStateTracker,
}

impl ProgressObserver {
    pub fn new(
        callback: impl FnMut(OptimizationProgressEvent) -> bool + Send + 'static,
        cancel_flag: Arc<AtomicBool>,
        free_vars: Vec<FreeVariable>,
        couplings: Vec<ParameterCoupling>,
        best_tracker: BestStateTracker,
    ) -> Self {
        Self {
            callback: Box::new(callback),
            cancel_flag,
            free_vars,
            couplings,
            best_tracker,
        }
    }
}

impl<I> Observe<I> for ProgressObserver
where
    I: State<Param = Vec<f64>, Float = f64>,
{
    fn observe_iter(&mut self, state: &I, _kv: &KV) -> Result<(), Error> {
        let param_vec = state.get_best_param().or_else(|| state.get_param());

        // Bisherigen Best-Zustand für einen vorzeitigen Abort sichern
        if let Some(p) = param_vec {
            if let Ok(mut lock) = self.best_tracker.lock() {
                *lock = Some((p.clone(), state.get_best_cost(), state.get_iter()));
            }
        }

        let current_params = if let Some(p) = param_vec {
            ParameterResolver::resolve(&self.free_vars, &self.couplings, p)
                .unwrap_or_default()
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };

        let event = OptimizationProgressEvent::Iteration {
            iteration: state.get_iter(),
            cost: state.get_cost(),
            best_cost: state.get_best_cost(),
            current_params,
        };

        let keep_running = (self.callback)(event);
        if self.cancel_flag.load(Ordering::Relaxed) || !keep_running {
            return Err(OpossumError::Other("Optimization cancelled by user".to_string()).into());
        }
        Ok(())
    }
}
