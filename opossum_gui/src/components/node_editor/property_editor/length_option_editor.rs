use crate::components::node_editor::{
    inputs::input_components::{LabeledInput, LabeledSelect},
    CallbackWrapper,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{nanometer, Proptype};
use uom::si::length::nanometer;

#[component]
pub fn AlignmentWavelengthEditor(property_key: String, prop_type_sig: Signal<Proptype>) -> Element {
    let select_id = format!("lengthProperty{property_key}").to_camel_case();
    let select_label = property_key.to_sentence_case();

    let (has_length, length_opt) = use_memo(move || {
        if let Proptype::LengthOption(Some(length)) = &*prop_type_sig.read() {
            (true, Some(*length))
        } else {
            (false, None)
        }
    })();

    rsx! {
        LabeledSelect {
            id: select_id,
            label: select_label,
            options: vec![(!has_length, "None".to_owned()), (has_length, "Define".to_owned())],
            onchange: move |_: Event<FormData>| {
                if has_length {
                    prop_type_sig.set(Proptype::LengthOption(None));
                } else {
                    prop_type_sig.set(Proptype::LengthOption(Some(nanometer!(1054.))));
                }
            },
        }
        {
            length_opt
                .map_or(
                    rsx! {},
                    |length| {
                        rsx! {
                            LabeledInput {
                                id: format!("lengthOptionProperty{property_key}").to_camel_case(),
                                label: format!("{} in nm", property_key.to_sentence_case()),
                                value: format!("{:.3}", length.get::<nanometer>()),
                                r#type: "number",
                                onchange: CallbackWrapper::new(move |e: Event<FormData>| {
                                    if let Ok(length) = e.data.value().parse::<f64>() {
                                        prop_type_sig.set(Proptype::LengthOption(Some(nanometer!(length))));
                                    }
                                }),
                            }
                        }
                    },
                )
        }
    }
}
