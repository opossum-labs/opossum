use argmin::core::{Error, KV, State, observers::Observe};
use opossum_core::error::OpossumError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::{
    address::ParameterAddress, coupling::ParameterCoupling, resolver::ParameterResolver,
    variable::FreeVariable,
};

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
        current_params: HashMap<ParameterAddress, f64>,
    },
    Finished {
        final_cost: f64,
        iterations: u64,
        converged: bool,
        optimized_parameters: HashMap<ParameterAddress, f64>,
    },
    Aborted {
        last_cost: f64,
        iterations: u64,
        best_parameters: HashMap<ParameterAddress, f64>,
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
}

impl ProgressObserver {
    pub fn new(
        callback: impl FnMut(OptimizationProgressEvent) -> bool + Send + 'static,
        cancel_flag: Arc<AtomicBool>,
        free_vars: Vec<FreeVariable>,
        couplings: Vec<ParameterCoupling>,
    ) -> Self {
        Self {
            callback: Box::new(callback),
            cancel_flag,
            free_vars,
            couplings,
        }
    }
}

impl<I> Observe<I> for ProgressObserver
where
    I: State<Param = Vec<f64>, Float = f64>,
{
    fn observe_iter(&mut self, state: &I, _kv: &KV) -> Result<(), Error> {
        let param_vec = state.get_best_param().or_else(|| state.get_param());
        let current_params = if let Some(p) = param_vec {
            ParameterResolver::resolve(&self.free_vars, &self.couplings, p).unwrap_or_default()
        } else {
            HashMap::new()
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
