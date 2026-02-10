use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop,
    inputs::input_components::{LabeledSelect, NodeConfigUnitInput},
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{meter, nanometer};
use uom::si::f64::Length;
use uuid::Uuid;

#[component]
pub fn LengthOptionEditor(
    node_id: Memo<Uuid>,
    length_opt: Option<Length>,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut length_opt_sig = use_signal(|| length_opt);

    use_effect({
        let property_key = property_key.to_owned();
        move || {
            if length_opt != *length_opt_sig.read() {
                on_change
                                        .call(NodeChangeEvent {
                                            node_id: *node_id.read(),
                                            action: NodeChangeAction::Property(
                                                property_key.clone(),
                                                (*length_opt_sig.read()).into(),
                                            ),
                                        });
            }
        }
    });

    let select_id = format!("lengthOptionProperty{property_key}").to_camel_case();
    let select_label = property_key.to_sentence_case();
    rsx! {
        LabeledSelect {
            id: select_id,
            label: select_label,
            options: vec![
                (length_opt_sig.read().is_none(), "None".to_owned()),
                (length_opt_sig.read().is_some(), "Define".to_owned()),
            ],
            onchange: move |_: Event<FormData>| {
                if length_opt_sig.read().is_some() {
                    length_opt_sig.set(None);
                } else {
                    length_opt_sig.set(Some(nanometer!(1054.)));
                }
            },
        }
        {
            length_opt_sig
                .read()
                .map_or(
                    rsx! {},
                    |length| {
                        rsx! {
                            NodeConfigUnitInput {
                                id: format!("lengthOptionProperty{property_key}").to_camel_case().as_str(),
                                label: property_key.to_sentence_case(),
                                value: length.value,
                                base_unit: "m",
                                onchange: move |new_length: f64| {
                                    length_opt_sig.set(Some(meter!(new_length)));
                                },
                            }
                        }
                    },
                )
        }
    }
}
