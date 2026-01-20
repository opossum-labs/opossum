use crate::components::node_editor::{
    CallbackWrapper, inputs::input_components::LabeledInput,
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::degree;
use uom::si::{angle::degree, f64::Angle};
use uuid::Uuid; // Import hinzugefügt

#[component]
pub fn AngleEditor(
    node_id: Uuid, // Prop hinzugefügt
    angle: Angle, 
    property_key: String
) -> Element {
    let angle_sig = use_signal(|| angle);

    // node_id an den Hook übergeben
    use_set_node_change_property(node_id, &property_key, angle, angle_sig);

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