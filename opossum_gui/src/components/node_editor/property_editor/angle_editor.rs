use crate::components::node_editor::{inputs::input_components::LabeledInput, CallbackWrapper};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{degree, Proptype};
use uom::si::{angle::degree, f64::Angle};

#[component]
pub fn AngleEditor(angle: Angle, property_key: String, prop_type_sig: Signal<Proptype>) -> Element {
    rsx! {
        LabeledInput {
            id: format!("angleProperty{property_key}").to_camel_case(),
            label: format!("{} angle in degrees", property_key.to_sentence_case()),
            value: format!("{}", angle.get::<degree>()),
            r#type: "number",
            onchange: on_angle_input_change(prop_type_sig),
        }
    }
}

fn on_angle_input_change(mut signal: Signal<Proptype>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(angle) = e.data.value().parse::<f64>() {
            signal.set(Proptype::Angle(degree!(angle)));
        }
    })
}
