use crate::components::node_editor::{
    hooks::use_synced_signal,
    inputs::input_components::{NodeConfigUnitInput, UnitHandling},
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::on_save_proptype_handler,
};
use approx::relative_ne;
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::degree;
use uom::si::{angle::degree, f64::Angle};
use uuid::Uuid;

#[component]
pub fn AngleEditor(
    node_id: ReadSignal<Uuid>,
    angle: Angle,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let angle_sig = use_synced_signal(angle);
    let on_save = on_save_proptype_handler(angle_sig, property_key.clone(), on_change, node_id);

    rsx! {
        NodeConfigUnitInput {
            id: format!("angleProperty{property_key}").to_camel_case().as_str(),
            label: property_key.to_sentence_case(),
            value: angle_sig.read().get::<degree>(),
            unit_config: UnitHandling::new("°", true),
            readonly,
            onchange: move |new_angle: f64| {
                if relative_ne!(angle_sig.read().get::< degree > (), new_angle, epsilon = 0.0) {
                    on_save.call(degree!(new_angle));
                }
            },
        }
    }
}
