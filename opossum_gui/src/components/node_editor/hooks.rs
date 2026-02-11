use dioxus::prelude::*;

pub struct SaveManager {
    pub flush_trigger: Signal<usize>,
    pub dirty_count: Signal<usize>,
}

/// Manages the save protocol for forms.
/// Returns the `flush_trigger` and `dirty_count` signals.
pub fn use_save_manager() -> SaveManager {
    let flush_trigger = use_signal(|| 0usize);
    let dirty_count = use_signal(|| 0usize);

    SaveManager {
        flush_trigger,
        dirty_count,
    }
}
