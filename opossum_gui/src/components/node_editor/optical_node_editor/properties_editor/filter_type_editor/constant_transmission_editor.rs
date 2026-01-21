use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        hooks::use_update_signal_with_reactive_prop, inputs::input_components::LabeledInput,
    },
};
use approx::relative_ne;
use dioxus::prelude::*;
use opossum_core::prelude::{Property, Proptype};

#[component]
pub fn ConstantFilterTypeEditor<T: From<f64> + PartialEq + Into<Proptype> + 'static>(
    transmission: f64,
    builder_sig: Signal<T>,
) -> Element {
    let mut transmission_sig = use_signal(|| transmission);
    let property = use_context::<Property>();

    use_update_signal_with_reactive_prop(transmission, transmission_sig);

    use_effect(move || {
        if relative_ne!(transmission, *transmission_sig.read()) {
            builder_sig.set((*transmission_sig.read()).into());
        }
    });
    rsx! {
        LabeledInput {
            id: "constFilterTypeInput",
            label: "Transmission",
            value: format!("{:.3}", transmission_sig.read()),
            r#type: "number",
            step: Some("0.01"),
            min: Some("0."),
            max: Some("1."),
            onchange: move |e: Event<FormData>| {
                if let Ok(val) = e.data.value().parse::<f64>() {
                    let last_val = *transmission_sig.read();
                    match property.validate_proptype(&T::from(val).into()) {
                        Ok(()) => transmission_sig.set(val),
                        Err(e) => {
                            OPOSSUM_UI_LOGS.write().add_log(format!("{e}").as_str());
                            transmission_sig.set(last_val);
                        }
                    }
                }
            },
        }
    }
}
