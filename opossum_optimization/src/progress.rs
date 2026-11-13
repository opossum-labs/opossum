use argmin::core::{Error, KV, State, observers::Observe};
use opossum_core::{error::OpossumError, opm_document::optimization::OptimizationProgressEvent};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
pub type BestStateTracker = Arc<Mutex<Option<(Vec<f64>, f64, u64)>>>;

pub struct ProgressObserver {
    callback: Box<dyn FnMut(OptimizationProgressEvent) -> bool + Send>,
    cancel_flag: Arc<AtomicBool>,
    best_tracker: BestStateTracker,
}

impl ProgressObserver {
    pub fn new(
        callback: impl FnMut(OptimizationProgressEvent) -> bool + Send + 'static,
        cancel_flag: Arc<AtomicBool>,
        best_tracker: BestStateTracker,
    ) -> Self {
        Self {
            callback: Box::new(callback),
            cancel_flag,
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
        if let Some(p) = param_vec
            && let Ok(mut lock) = self.best_tracker.lock()
        {
            *lock = Some((p.clone(), state.get_best_cost(), state.get_iter()));
        }
        let event = OptimizationProgressEvent::Iteration {
            iteration: state.get_iter(),
            best_cost: state.get_best_cost(),
        };

        let keep_running = (self.callback)(event);
        if self.cancel_flag.load(Ordering::Relaxed) || !keep_running {
            return Err(OpossumError::Other("Optimization cancelled by user".to_string()).into());
        }
        Ok(())
    }
}
