use crate::components::node_editor::{
    CallbackWrapper,
    inputs::input_components::LabeledInput,
    node_config_editor::NodeChangeEvent, // Import hinzugefügt
    optical_node_editor::properties_editor::{
        use_set_node_change_property,
        use_update_signal_with_reactive_prop, // Helper Import hinzugefügt
    },
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::degree;
use uom::si::{angle::degree, f64::Angle};
use uuid::Uuid;

#[component]
pub fn AngleEditor(
    node_id: Uuid,
    angle: Angle,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let angle_sig = use_signal(|| angle);
    let bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);
    use_set_node_change_property(
        *bound_node_id.read(),
        &property_key,
        angle,
        angle_sig,
        on_change,
    );
    rsx! {
        LabeledInput {
            id: format!("angleProperty{property_key}").to_camel_case(),
            label: format!("{} angle in degrees", property_key.to_sentence_case()),
            value: format!("{:.3}", angle_sig.read().get::<degree>()),
            r#type: "number",
            onchange: on_angle_input_change(angle_sig),
        }
    }
}

fn on_angle_input_change(mut signal: Signal<Angle>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(angle) = e.data.value().parse::<f64>() {
            signal.set(degree!(angle));
        }
    })
}
