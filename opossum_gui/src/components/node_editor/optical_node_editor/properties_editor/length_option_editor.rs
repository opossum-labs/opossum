use crate::components::node_editor::{
    hooks::use_synced_signal,
    inputs::input_components::{LabeledSelect, NodeConfigUnitInput, UnitHandling},
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::on_save_proptype_handler,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{meter, nanometer};
use uom::si::f64::Length;
use uuid::Uuid;

#[component]
pub fn LengthOptionEditor(
    node_id: ReadSignal<Uuid>,
    length_opt: Option<Length>,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let length_opt_sig = use_synced_signal(length_opt);
    let on_save =
        on_save_proptype_handler(length_opt_sig, property_key.clone(), on_change, node_id);

    rsx! {
        LabeledSelect {
            id: format!("lengthOptionProperty{property_key}").to_camel_case(),
            label: property_key.to_sentence_case(),
            options: vec![
                (length_opt_sig.read().is_none(), "None".to_owned()),
                (length_opt_sig.read().is_some(), "Define".to_owned()),
            ],
            readonly,
            onchange: move |_: Event<FormData>| {
                if length_opt_sig.read().is_some() {
                    on_save.call(None);
                } else {
                    on_save.call(Some(nanometer!(1054.)));
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
                                unit_config: UnitHandling::new("m", true),
                                readonly,
                                onchange: move |new_length: f64| {
                                    on_save.call(Some(meter!(new_length)));
                                },
                            }
                        }
                    },
                )
        }
    }
}
