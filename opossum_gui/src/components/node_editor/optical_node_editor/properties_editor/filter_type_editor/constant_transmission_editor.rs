use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        hooks::use_update_signal_with_reactive_prop, inputs::input_components::FlushableTextInput,
    },
};
use approx::relative_ne;
use dioxus::prelude::*;
use opossum_core::{nodes::SplittingConfigBuilder, prelude::{Property, Proptype}};


#[component]
pub fn ConstantFilterTypeEditor(
    transmission: f64,
    on_transmission_change: EventHandler<SplittingConfigBuilder>,
) -> Element {
    let transmission_sig = use_signal(|| transmission);
    rsx! {
        FlushableTextInput {
            id: "constFilterTypeInput".to_string(),
            label: "Transmission".to_string(),
            value: format!("{:.3}", transmission_sig.read()),
            on_save: move |new_val: String| {
                let old_val = *transmission_sig.read();
                if let Ok(val) = new_val.parse::<f64>()
                    && relative_ne!(old_val, val)
                {
                    on_transmission_change.call(SplittingConfigBuilder::from(val));
                }
            },
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
        }

    }
}
