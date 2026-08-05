#![allow(clippy::derive_partial_eq_without_eq)]
use dioxus::prelude::*;
use opossum_core::types::api_types::NodeEditorPanel;

#[component]
pub fn AccordionItem(
    elements: Vec<Element>,
    header: String,
    header_id: String,
    parent_id: String,
    content_id: String,
    level: usize,
) -> Element {
    rsx! {
        div { class: "accordion-item bg-dark text-light",
            h6 { class: "accordion-header", id: &header_id,
                button {
                    class: "accordion-button accordion-button-h{level} collapsed bg-dark text-light",
                    r#type: "button",
                    "data-mdb-collapse-init": "",
                    "data-mdb-target": format!("#{content_id}"),
                    "aria-expanded": "false",
                    "aria-controls": &content_id,
                    {header}
                }
            }
            div {
                id: &content_id,
                class: "accordion-collapse collapse  bg-dark",
                "aria-labelledby": &header_id,
                "data-mdb-parent": format!("#{parent_id}"),
                div { class: "accordion-body  bg-dark",
                    for element in elements {
                        {element}
                    }
                }
            }
        }
    }
}

#[component]
pub fn ElementList(element_list: Vec<Element>) -> Element {
    rsx! {
        div { class: "bg-dark",
            for element in element_list {
                {element}
            }
        }
    }
}

/// The `content_id` each panel's `AccordionItem` was built with (see `general_editor.rs`,
/// `port_config_editor/mod.rs`, `properties_editor/mod.rs`, `alignment_editor/mod.rs`'s
/// `PositioningEditor`/`AlignmentEditor`) - the *only* place these 5 strings are written down; every
/// `AccordionItem`'s own `content_id` prop is built from this function too, so the two can never
/// drift apart.
pub const fn content_id_for_panel(panel: NodeEditorPanel) -> &'static str {
    match panel {
        NodeEditorPanel::General => "generalCollapse",
        NodeEditorPanel::PortConfig => "portConfigCollapse",
        NodeEditorPanel::Properties => "propertyCollapse",
        NodeEditorPanel::Positioning => "positionCollapse",
        NodeEditorPanel::Alignment => "alignmentCollapse",
    }
}

/// Opens the accordion section `panel` maps to. See [`open_accordion_content`] for the JS-interop shape;
/// since all 5 top-level sections share `data-mdb-parent="#accordionNodeConfig"`, `.show()` alone also
/// closes whichever section was open - true accordion behavior needs nothing extra.
pub fn open_accordion_section(panel: NodeEditorPanel) {
    open_accordion_content(content_id_for_panel(panel));
}

/// Opens (`.show()`) the collapsible whose element id is `content_id`, mirroring
/// `menu_bar_component::hide_dropdown`'s JS-interop shape (same mdb-ui-kit static-instance pattern,
/// `Collapse`/`.show()` instead of `Dropdown`/`.hide()`). `getOrCreateInstance(el, { toggle: false })`
/// avoids MDB's constructor-time auto-toggle. Used both for the top-level sections (via
/// [`open_accordion_section`]) and for the nested per-port sub-accordions inside Port Configuration.
pub fn open_accordion_content(content_id: &str) {
    let content_id = content_id.to_string();
    let script = format!(
        r"
        const el = document.getElementById('{content_id}');
        if (el) {{
            const instance = mdb.Collapse.getOrCreateInstance(el, {{ toggle: false }});
            instance.show();
        }}
    "
    );
    spawn(async move {
        let _ = dioxus::document::eval(&script).await;
    });
}
