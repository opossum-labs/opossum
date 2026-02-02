use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop, inputs::input_components::{LabeledInput, NodeConfigUnitInput},
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use approx::relative_ne;
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
    let mut angle_sig = use_signal(|| angle);
    use_update_signal_with_reactive_prop(angle, angle_sig);
    let value_memo = use_memo(move || angle_sig.read().get::<degree>());
    rsx! {
        NodeConfigUnitInput {
            id: format!("angleProperty{property_key}").to_camel_case().as_str(),
            label: property_key.to_sentence_case(),
            value: value_memo,
            base_unit: "°",
            onchange: move |new_angle: f64| {
                if relative_ne!(angle_sig.read().get::< degree > (), new_angle) {
                    angle_sig.set(degree!(new_angle % 360.0));
                    on_change
                        .call(NodeChangeEvent {
                            node_id,
                            action: NodeChangeAction::Property(
                                property_key.clone(),
                                degree!(new_angle % 360.0).into(),
                            ),
                        });
                }
            },
        }
    }
}
