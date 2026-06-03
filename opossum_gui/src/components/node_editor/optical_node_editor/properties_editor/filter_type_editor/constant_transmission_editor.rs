use crate::{
    OPOSSUM_UI_LOGS, components::node_editor::inputs::input_components::NodeConfigPlainF64Input,
};
use approx::relative_ne;
use dioxus::prelude::*;

#[component]
pub fn ConstantFilterTypeEditor<T>(
    transmission: f64,
    on_transmission_change: EventHandler<T>,
    readonly: bool,
) -> Element
where
    T: TryFrom<f64> + PartialEq + Clone + 'static,
{
    let transmission_sig = use_signal(|| transmission);

    rsx! {
        NodeConfigPlainF64Input {
            id: "constFilterTypeInput".to_string(),
            label: "Transmission".to_string(),
            value: transmission_sig,
            readonly,
            onchange: move |new_val: f64| {
                if relative_ne!(* transmission_sig.read(), new_val, epsilon = 0.0) {
                    // Use try_from instead of from
                    if let Ok(converted) = T::try_from(new_val) {
                        on_transmission_change.call(converted);
                    } else {
                        OPOSSUM_UI_LOGS
                            .write()
                            .add_log(
                                &format!("Invalid input value for transmission: {new_val}"),
                            );
                    }
                }
            },
        }
    }
}
