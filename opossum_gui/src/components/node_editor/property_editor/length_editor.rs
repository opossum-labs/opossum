use crate::components::node_editor::{
    inputs::input_components::{LabeledInput, LabeledSelect},
    CallbackWrapper,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{millimeter, Proptype};
use uom::si::{f64::Length, length::millimeter};

#[component]
pub fn LengthEditor(
    length: Length,
    property_key: String,
    prop_type_sig: Signal<Proptype>,
) -> Element {
    let select_id = format!("lengthProperty{property_key}").to_camel_case();
    let select_label = property_key.to_sentence_case();
    let length_finite_sig = use_memo(move || {
        if let Proptype::Length(length) = &*prop_type_sig.read() {
            length.clone()
        } else {
            millimeter!(1000.)
        }
    });

    rsx! {
        LabeledSelect {
            id: select_id,
            label: select_label,
            options: vec![
                (length_finite_sig().is_infinite(), "Flat".to_owned()),
                (length_finite_sig().is_finite(), "Curved".to_owned()),
            ],
            onchange: move |_: Event<FormData>| {
                if length_finite_sig.read().is_finite() {
                    prop_type_sig.set(millimeter!(f64::INFINITY).into());
                } else {
                    prop_type_sig.set(millimeter!(1000.).into());
                }
            },
        }
        {
            if length_finite_sig().is_finite() {
                rsx! {
                    LabeledInput {
                        id: format!("lengthProperty{property_key}").to_camel_case(),
                        label: format!("{} in mm", property_key.to_sentence_case()),
                        value: format!("{}", length_finite_sig().get::<millimeter>()),
                        r#type: "number",
                        onchange: on_length_input_change(prop_type_sig),
                    }
                }
            } else {
                rsx! {}
            }
        }
    }
}

fn on_length_input_change(mut signal: Signal<Proptype>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(length) = e.data.value().parse::<f64>() {
            signal.set(millimeter!(length).into());
        }
    })
}
