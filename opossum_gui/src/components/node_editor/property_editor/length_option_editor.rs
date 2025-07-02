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
    let mut alignment_select = Signal::new(nanometer!(1054.));
    let select_id = format!("lengthProperty{property_key}").to_camel_case();
    let select_label = property_key.to_sentence_case();

    if let Proptype::LengthOption(Some(length)) = &*prop_type_sig.read() {
        rsx! {
            LabeledSelect {
                id: select_id,
                label: select_label,
                options: vec![
                    (false, "As in light definition".to_owned()),
                    (true, "Choose specific".to_owned()),
                ],
                onchange: move |_: Event<FormData>| prop_type_sig.set(Proptype::LengthOption(None)),
            }
            LabeledInput {
                id: format!("lengthOptionProperty{property_key}").to_camel_case(),
                label: format!("{} in nm", property_key.to_sentence_case()),
                value: format!("{}", length.get::<nanometer>()),
                r#type: "number",
                onchange: CallbackWrapper::new(move |e: Event<FormData>| {
                    if let Ok(length) = e.data.value().parse::<f64>() {
                        prop_type_sig.set(Proptype::LengthOption(Some(nanometer!(length))));
                        alignment_select.set(nanometer!(length));
                    }
                }),
            }
        }
    } else {
        rsx! {
            LabeledSelect {
                id: select_id,
                label: select_label,
                options: vec![
                    (true, "As in light definition".to_owned()),
                    (false, "Choose specific".to_owned()),
                ],
                onchange: move |_: Event<FormData>| {
                    prop_type_sig.set(Proptype::LengthOption(Some(alignment_select())));
                },
            }
        }
    }
}
