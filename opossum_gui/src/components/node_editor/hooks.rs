use dioxus::prelude::*;

/// Synchronizes a signal with a reactive property.
/// If the property changes (e.g. from a parent component), the signal is updated.
pub fn use_update_signal_with_reactive_prop<T: PartialEq + Clone + 'static>(
    prop: T,
    mut prop_signal: Signal<T>,
) {
    use_effect(use_reactive!(|(prop,)| {
        if *prop_signal.peek() != prop {
            prop_signal.set(prop);
        }
    }));
}

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
