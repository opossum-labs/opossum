use crate::components::node_editor::inputs::input_components::NodeConfigPlainF64Input;
use approx::relative_ne;
use dioxus::prelude::*;

#[component]
pub fn ConstantFilterTypeEditor<T: From<f64> + PartialEq + Clone + 'static>(
    transmission: f64,
    on_transmission_change: EventHandler<T>,
) -> Element {
    let transmission_sig = use_signal(|| transmission);
    rsx! {
        NodeConfigPlainF64Input {
            id: "constFilterTypeInput".to_string(),
            label: "Transmission".to_string(),
            value: transmission_sig,
            onchange: move |new_val: f64| {
                if relative_ne!(* transmission_sig.read(), new_val, epsilon = 0.0) {
                    on_transmission_change.call(T::from(new_val));
                }
            },
        }

    }
}
