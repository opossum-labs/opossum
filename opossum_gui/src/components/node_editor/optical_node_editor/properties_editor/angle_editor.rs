use crate::components::node_editor::{
    inputs::input_components::NodeConfigUnitInput,
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
use approx::relative_ne;
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::degree;
use uom::si::{angle::degree, f64::Angle};
use uuid::Uuid;

#[component]
pub fn AngleEditor(
    node_id: Memo<Uuid>,
    angle: Angle,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let angle_memo = use_memo(use_reactive!(|angle| angle.get::<degree>()));
    rsx! {
        NodeConfigUnitInput {
            id: format!("angleProperty{property_key}").to_camel_case().as_str(),
            label: property_key.to_sentence_case(),
            value: angle_memo(),
            base_unit: "°",
            onchange: move |new_angle: f64| {
                if relative_ne!(angle_memo(), new_angle) {
                    on_change
                        .call(NodeChangeEvent {
                            node_id: *node_id.read(),
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
