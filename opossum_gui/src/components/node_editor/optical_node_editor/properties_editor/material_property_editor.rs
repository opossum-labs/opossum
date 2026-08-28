use crate::components::{
    asset_editor::material_editor::{MaterialChangeEvent, MaterialEditor},
    catalog_editor::MaterialCatalog,
    node_editor::node_config_editor::{NodeChangeAction, NodeChangeEvent},
    primitives::button::{Button, ButtonSize, ButtonVariant},
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaBook, FaCloudArrowUp, FaLinkSlash, FaPencil},
};
use opossum_core::{material::Material, properties::proptype::AssetRef};
use opossum_registry::AssetRegistry;
use uuid::Uuid;

/// Component for viewing, editing, and assigning optical materials to a node.
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
    // Access global asset loader from Dioxus context
    let mut registry = use_context::<Signal<AssetRegistry<Material>>>();

    // Extract current material properties
    let current_material = material_ref.unwrap_inline().clone();
    let is_catalog = current_material.version() > 0;
    let material_name = current_material.name().to_string();
    let current_version = current_material.version();

    // Modal dialog visibility states
    let mut show_catalog_dialog = use_signal(|| false);
    let mut show_editor_dialog = use_signal(|| false);

    // Local state buffer for inline material editing
    let mut editing_material = use_signal(|| current_material.clone());

    // Helper closure to emit property change events back to the node graph
    let emit_material_change = {
        let property_key = property_key.clone();
        move |updated_material: Material| {
            on_change.call(NodeChangeEvent {
                node_id: *node_id.read(),
                action: NodeChangeAction::Property(
                    property_key.clone(),
                    AssetRef::Inline(updated_material).into(),
                ),
            });
        }
    };

    // Callback: Material selected from the catalog selector
    let on_catalog_select = {
        let emit_material_change = emit_material_change.clone();
        use_callback(move |selected_mat: Material| {
            info!("Selected material from catalog: {}", selected_mat.name());
            emit_material_change(selected_mat);
        })
    };

    // Callback: Unlink catalog material into an independent AdHoc draft
    let on_unlink_to_adhoc = {
        let emit_material_change = emit_material_change.clone();
        let current_material = current_material.clone();
        use_callback(move |_| {
            info!(
                "Unlinking material '{}' to AdHoc draft...",
                current_material.name()
            );
            let adhoc_copy = current_material.clone_as_adhoc();
            emit_material_change(adhoc_copy);
        })
    };

    // Callback: Publish AdHoc material into the catalog registry
    let on_publish_adhoc_to_catalog = {
        let emit_material_change = emit_material_change.clone();
        let mut current_material = current_material.clone();
        use_callback(move |_| {
            info!(
                "Publishing AdHoc material '{}' to registry...",
                current_material.name()
            );
            match registry.write().publish(&mut current_material) {
                Ok(_) => emit_material_change(current_material.clone()),
                Err(err) => log::error!("Failed to publish: {err}"),
            }
        })
    };

    // Callbacks for inline AdHoc MaterialEditor
    let on_inline_editor_change = use_callback(move |evt: MaterialChangeEvent| {
        evt.action.apply(&mut editing_material.write());
    });

    let on_inline_editor_save = {
        let emit_material_change = emit_material_change.clone();
        use_callback(move |()| {
            let updated = editing_material.read().clone();
            emit_material_change(updated);
            show_editor_dialog.set(false);
        })
    };

    rsx! {

        div { class: "form-floating border-start",
            div { class: "form-control form-control-sm material-prop-display",
                span {
                    class: "material-prop-name text-truncate",
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
                    span { class: "badge bg-secondary flex-shrink-0", "AdHoc" }
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
