use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        inputs::input_components::LabeledInput,
        node_config_editor::NodeChangeEvent,
        optical_node_editor::properties_editor::{
            use_set_node_change_property, use_update_signal_with_reactive_prop,
        },
    },
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::prelude::{Property, Proptype};
use uuid::Uuid;

#[component]
pub fn F64Editor(
    node_id: Uuid,
    float64: f64,
    property_key: String,
    property: Property,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut float64_sig = use_signal(|| float64);
    let bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);
    use_set_node_change_property(
        *bound_node_id.read(),
        &property_key,
        float64,
        float64_sig,
        on_change,
    );
    rsx! {
        LabeledInput {
            id: format!("float64Property{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: format!("{:.3}", *float64_sig.read()),
            r#type: "number",
            onchange: move |e: Event<FormData>| {
                if let Ok(val) = e.data.value().parse::<f64>() {
                    let last_val = *float64_sig.read();
                    match property.validate_proptype(&Proptype::F64(val)) {
                        Ok(()) => float64_sig.set(val),
                        Err(e) => {
                            OPOSSUM_UI_LOGS.write().add_log(format!("{e}").as_str());
                            float64_sig.set(last_val);
                        }
                    }
                }
            },
        }
    }
}
