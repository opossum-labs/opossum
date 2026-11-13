use crate::opm_document::optimization::ParameterAddress;
use serde::{Deserialize, Serialize};

/// Structure for transmitting optimization progress events (through backend API).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum OptimizationProgressEvent {
    /// The optimization has been started.
    Started {
        /// Initial cost value before the first solver step.
        initial_cost: f64,
    },
    /// An iteration step was performed.
    Iteration {
        /// Current iteration step.
        iteration: u64,
        /// Best cost value achieved so far.
        best_cost: f64,
    },
    /// The optimization has finished normally.
    Finished {
        /// Final best cost value.
        final_cost: f64,
        /// Optimized parameter values applied to the document.
        optimized_parameters: Vec<(ParameterAddress, f64)>,
    },
    /// The optimization was aborted.
    Aborted {
        /// Best cost value achieved before aborting (may be `NaN` if aborted at step 0).
        last_cost: f64,
        /// Best set of parameters achieved so far.
        best_parameters: Vec<(ParameterAddress, f64)>,
    },
    /// The optimization produced an error.
    Failed {
        /// Error message.
        error: String,
    },
}
