use crate::components::node_editor::{
    accordion::AccordionItem,
    node_editor_component::{NodeChange, NodePropInput},
};
use dioxus::{html::geometry::euclid::num::Zero, prelude::*};
use opossum_backend::Isometry;
use uom::si::f64::{Angle, Length};

#[component]
pub fn AlignmentEditor(alignment: Option<Isometry>) -> Element {
    let accordion_content = vec![rsx! {
        NodePropInput {
            name: "XTranslation".to_string(),
            placeholder: "X Translation in m".to_string(),
            node_change: NodeChange::TranslationX(
                alignment.as_ref().map_or(Length::zero(), |a| a.translation().x),
            ),
        }
        NodePropInput {
            name: "YTranslation".to_string(),
            placeholder: "Y Translation in m".to_string(),
            node_change: NodeChange::TranslationY(
                alignment.as_ref().map_or(Length::zero(), |a| a.translation().y),
            ),
        }
        NodePropInput {
            name: "ZTranslation".to_string(),
            placeholder: "Z Translation in m".to_string(),
            node_change: NodeChange::TranslationZ(
                alignment.as_ref().map_or(Length::zero(), |a| a.translation().z),
            ),
        }
        NodePropInput {
            name: "Roll".to_string(),
            placeholder: "Roll angle in degree".to_string(),
            node_change: NodeChange::RotationRoll(
                alignment.as_ref().map_or(Angle::zero(), |a| a.rotation().x),
            ),
        }
        NodePropInput {
            name: "Pitch".to_string(),
            placeholder: "Pitch angle in degree".to_string(),
            node_change: NodeChange::RotationPitch(
                alignment.as_ref().map_or(Angle::zero(), |a| a.rotation().y),
            ),
        }
        NodePropInput {
            name: "Yaw".to_string(),
            placeholder: "Yaw angle in degree".to_string(),
            node_change: NodeChange::RotationYaw(
                alignment.as_ref().map_or(Angle::zero(), |a| a.rotation().z),
            ),
        }
    }];
    rsx! {
        AccordionItem {
            elements: accordion_content,
            header: "Alignment",
            header_id: "alignmentHeading",
            parent_id: "accordionNodeConfig",
            content_id: "alignmentCollapse",
        }
    }
}
