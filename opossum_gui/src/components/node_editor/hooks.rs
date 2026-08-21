use dioxus::prelude::*;

/// Keeps a local signal resynced to an upstream prop value across renders.
///
/// Plain `use_signal(|| some_prop)` only runs its initializer on first mount - it never picks up a
/// fresh `some_prop` value later (e.g. after an undo/redo-triggered refetch upstream), even though the
/// component's own function body correctly re-runs with the new prop each time (Dioxus's props
/// memoization compares plain, non-signal prop fields by value). `use_effect(use_reactive!(...))` is
/// *not* a reliable fix here either: it only re-checks its dependency when the component function
/// re-runs, but in practice this proved unreliable across renders triggered by sibling-signal writes
/// (e.g. `is_locally_dirty`) rather than by `value` itself changing. Comparing directly in the
/// component body, as this hook does, sidesteps both problems.
///
/// Returns a `Signal<T>` seeded with `value` that also updates itself in place whenever a *new*
/// `value` is passed on a later call (typically because the prop changed upstream) - while still
/// allowing independent local mutation (e.g. via `.set()` in an `on_save` handler) between such syncs.
pub fn use_synced_signal<T: PartialEq + Clone + 'static>(value: T) -> Signal<T> {
    let mut local_value = use_signal(|| value.clone());
    let mut last_seen_value = use_signal(|| value.clone());
    if *last_seen_value.peek() != value {
        last_seen_value.set(value.clone());
        local_value.set(value);
    }
    local_value
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
