use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop,
    inputs::input_components::{LabeledInput, LabeledSelect, NodeConfigUnitInput},
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{meter, nanometer};
use uom::si::{f64::Length, length::nanometer};
use uuid::Uuid;

#[component]
pub fn LengthOptionEditor(
    node_id: Uuid,
    length_opt: Option<Length>,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut length_opt_sig = use_signal(|| length_opt);
    use_update_signal_with_reactive_prop(length_opt, length_opt_sig);
    
    let select_id = format!("lengthOptionProperty{property_key}").to_camel_case();
    let select_label = property_key.to_sentence_case();
    // let bound_node_id = use_signal(|| node_id);
    // use_update_signal_with_reactive_prop(node_id, bound_node_id);
    
    // use_set_node_change_property(
    //     *bound_node_id.read(),
    //     &property_key,
    //     length_opt,
    //     length_opt_sig,
    //     on_change,
    // );

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
                                    on_change
                                        .call(NodeChangeEvent {
                                            node_id,
                                            action: NodeChangeAction::Property(
                                                property_key.clone(),
                                                length_opt_sig.read().clone().into(),
                                            ),
                                        });
                                },
                            }
                        }
                    },
                )
        }
    }
}
