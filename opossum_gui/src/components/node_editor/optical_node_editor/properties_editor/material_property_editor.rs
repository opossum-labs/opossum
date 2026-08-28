use crate::components::{
    inputs::material_selector::MaterialSelector,
    node_editor::node_config_editor::{NodeChangeAction, NodeChangeEvent},
    primitives::button::{Button, ButtonSize, ButtonVariant},
};
use dioxus::prelude::*;
use opossum_core::{material::Material, properties::proptype::AssetRef};
use uuid::Uuid;

/// Component for viewing, editing, and assigning optical materials to a node property.
#[component]
pub fn MaterialPropertyEditor(
    /// ID of the node being edited.
    node_id: ReadSignal<Uuid>,
    /// Material reference property of the node.
    material_ref: AssetRef<Material>,
    /// Name/key of the property inside the node.
    property_key: String,
    /// Event handler to propagate changes back to the node graph.
    on_change: EventHandler<NodeChangeEvent>,
    /// Readonly flag to disable editing interactions.
    readonly: bool,
) -> Element {
    let current_material = material_ref.unwrap_inline().clone();
    let prop_key = property_key.clone();

    let on_material_change = move |updated_material: Material| {
        on_change.call(NodeChangeEvent {
            node_id: *node_id.read(),
            action: NodeChangeAction::Property(
                prop_key.clone(),
                AssetRef::Inline(updated_material).into(),
            ),
        });
    };

    rsx! {

        div { class: "form-floating border-start",
            div { class: "form-control form-control-sm material-prop-display",
                span {
                    class: "material-prop-name text-truncate ",
                    title: "{material_name}",
                    "{material_name}"
                }
                if is_catalog {
                    span { class: "badge bg-primary flex-shrink-0", "v{current_version}" }
                    if !readonly {
                        Button {
                            size: ButtonSize::IconXs,
                            variant: ButtonVariant::Primary,
                            title: "Choose a different material from the catalog",
                            class: "material-btn",
                            onclick: move |_| show_catalog_dialog.set(true),
                            Icon { icon: FaBook }
                        }
                        Button {
                            size: ButtonSize::IconXs,
                            variant: ButtonVariant::Secondary,
                            title: "Detach from catalog (create a local copy)",
                            class: "material-btn",
                            onclick: on_unlink_to_adhoc,
                            Icon { icon: FaLinkSlash }
                        }
                    }
                } else {
                    span { class: "material-btn badge flex-shrink-0", "AdHoc" }
                    if !readonly {
                        Button {
                            size: ButtonSize::IconXs,
                            variant: ButtonVariant::Primary,
                            title: "Edit local material properties",
                            class: "material-btn",
                            onclick: {
                                let mat = current_material.clone();
                                move |_| {
                                    editing_material.set(mat.clone());
                                    show_editor_dialog.set(true);
                                }
                            },
                            Icon { icon: FaPencil }
                        }
                        Button {
                            size: ButtonSize::IconXs,
                            variant: ButtonVariant::Success,
                            title: "Publish this AdHoc material into the permanent catalog",
                            class: "material-btn",
                            onclick: on_publish_adhoc_to_catalog,
                            Icon { icon: FaCloudArrowUp }
                        }
                        Button {
                            size: ButtonSize::IconXs,
                            variant: ButtonVariant::Primary,
                            title: "Replace with an existing catalog material",
                            class: "material-btn",
                            onclick: move |_| show_catalog_dialog.set(true),
                            Icon { icon: FaBook }
                        }
                    }
                }
            }
            label { class: "form-label text-secondary", "{property_key}" }
        }

        MaterialCatalog { open: show_catalog_dialog, on_select: on_catalog_select }
        MaterialEditor {
            open: show_editor_dialog,
            material: editing_material,
            readonly,
            on_change: on_inline_editor_change,
            on_save: on_inline_editor_save,
            save_label: "Save Changes".to_string(),
        }
    }
}
