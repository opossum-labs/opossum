use crate::components::node_editor::{
    inputs::input_components::LabeledInput, node_editor_component::NodeChange,
    property_editor::use_set_node_change_property, CallbackWrapper,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::degree;
use uom::si::{angle::degree, f64::Angle};

#[component]
pub fn AngleEditor(
    angle: Angle,
    property_key: String,
    node_change: Signal<Option<NodeChange>>,
) -> Element {
    let angle_sig = use_signal(|| angle);

    use_set_node_change_property(&property_key, angle, angle_sig, node_change);

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
