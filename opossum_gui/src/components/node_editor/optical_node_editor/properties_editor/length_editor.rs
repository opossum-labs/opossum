use crate::{
    components::node_editor::{
        hooks::use_synced_signal,
        inputs::input_components::{NodeConfigUnitInput, UnitHandling},
        node_config_editor::NodeChangeEvent,
        optical_node_editor::properties_editor::on_save_proptype_handler,
    },
    utils::ToSentenceCase,
};
use approx::relative_ne;
use dioxus::prelude::*;

use heck::ToLowerCamelCase;
use opossum_core::meter;
use uom::si::f64::Length;
use uuid::Uuid;

#[component]
pub fn LengthEditor(
    node_id: ReadSignal<Uuid>,
    length: Length,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let length_sig = use_synced_signal(length);
    let on_save = on_save_proptype_handler(length_sig, property_key.clone(), on_change, node_id);

    rsx! {
        NodeConfigUnitInput {
            id: format!("lengthProperty{property_key}").to_lower_camel_case().as_str(),
            label: property_key.to_sentence_case(),
            value: length_sig.read().value,
            unit_config: UnitHandling::new("m", true),
            readonly,
            onchange: move |new_length: f64| {
                if relative_ne!(length_sig.read().value, new_length, epsilon = 0.0) {
                    on_save.call(meter!(new_length));
                }
            },
        }
    }
}
