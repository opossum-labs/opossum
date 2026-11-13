use crate::opm_document::optimization::ParameterAddress;
use serde::{Deserialize, Serialize};

/// Structure for transmitting optimizaion progress events (through backend API)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum OptimizationProgressEvent {
    /// the optimization has been started
    Started {
        /// initial cost value
        initial_cost: f64,
        /// maximum total iterations
        total_max_iterations: u64,
    },
    /// an itereation step was performed
    Iteration {
        /// current iteration step
        iteration: u64,
        /// current cost value
        cost: f64,
        /// best cost value so far
        best_cost: f64,
        /// current parameter values
        current_params: Vec<(ParameterAddress, f64)>,
    },
    /// the optimization has normally finished
    Finished {
        /// final cost value
        final_cost: f64,
        /// final number of iterations needed
        iterations: u64,
        /// true if it converged
        converged: bool,
        /// optimized parameter values
        optimized_parameters: Vec<(ParameterAddress, f64)>,
    },
    /// the optimization was manually aborted
    Aborted {
        /// latest cost value
        last_cost: f64,
        /// iterations performed so far
        iterations: u64,
        /// best set of parameters so far
        best_parameters: Vec<(ParameterAddress, f64)>,
    },
    /// the optimization produced an error
    Failed {
        /// error message
        error: String,
    },
}
