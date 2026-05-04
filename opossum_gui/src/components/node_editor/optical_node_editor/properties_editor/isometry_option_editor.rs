use crate::components::node_editor::{
    accordion::AccordionItem,
    node_config_editor::NodeChangeEvent,
    optical_node_editor::{
        RotationAlignmentInputs, TranslationAlignmentInputs, on_new_rotation, on_new_translation,
        properties_editor::on_save_proptype_handler,
    },
};
use dioxus::prelude::*;
use opossum_core::prelude::Isometry;
use uuid::Uuid;

#[component]
pub fn IsometryOptionEditor(
    node_id: Memo<Uuid>,
    isometry: Isometry,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let isometry_sig = use_signal(|| isometry);
    let on_save = on_save_proptype_handler(isometry_sig, property_key, on_change, node_id.into());

    let position_memo = use_memo(move || *isometry_sig.read());

    let on_position_change = EventHandler::new(move |new_iso: Isometry| {
        on_save.call(new_iso);
    });

    let mut accordion_content = vec![];

    accordion_content.push(rsx! {
        RotationAlignmentInputs {
            alignment: position_memo,
            axes_skip: None,
            on_new_rotation: on_new_rotation(on_position_change, position_memo.into()),
            node_id,
            readonly,
        }
        TranslationAlignmentInputs {
            alignment: position_memo,
            axes_skip: None,
            on_new_translation: on_new_translation(on_position_change, position_memo.into()),
            node_id,
            readonly,
        }
    });

    rsx! {
        div {
            class: "accordion accordion-borderless bg-dark border-start",
            id: "accordionIsometryOptionConfig",
            AccordionItem {
                elements: accordion_content,
                header: "Source isometry",
                header_id: "srcIsometryHeading",
                parent_id: "accordionIsometryOptionConfig",
                content_id: "srcIsometryCollapse",
                level: 2,
            }
        }
    }
}
