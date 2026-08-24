use crate::components::{
    asset_editor::material_editor::{MaterialChangeEvent, MaterialEditor},
    catalog_editor::MaterialCatalog,
    node_editor::node_config_editor::{NodeChangeAction, NodeChangeEvent},
    primitives::button::{Button, ButtonVariant},
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
        div { class: "mb-3",
            // Property label header
            label { class: "form-label fw-bold small text-capitalize mb-1", "{property_key}" }

            // Row 1: Full-width container with material name and status badge
            div { class: "d-flex justify-content-between align-items-center px-2 py-1 rounded bg-dark border border-secondary",
                span {
                    class: "text-truncate fw-semibold small text-light",
                    title: "{material_name}",
                    "{material_name}"
                }
                if is_catalog {
                    span { class: "badge bg-primary flex-shrink-0 ms-2", "v{current_version}" }
                } else {
                    span { class: "badge bg-secondary flex-shrink-0 ms-2", "AdHoc" }
                }
            }

            // Row 2: Action buttons row
            if !readonly {
                div { class: "d-flex gap-1 mt-1 align-items-stretch",
                    if is_catalog {
                        // 1. Catalog mode actions: Two full-width buttons
                        div { class: "flex-fill",
                            Button {
                                title: "Choose a different material from the catalog",
                                onclick: move |_| show_catalog_dialog.set(true),
                                Icon { icon: FaBook }
                                "Change"
                            }
                        }
                        div { class: "flex-fill",
                            Button {
                                title: "Detach from catalog (create an independent local copy)",
                                variant: ButtonVariant::Secondary,
                                onclick: on_unlink_to_adhoc,
                                Icon { icon: FaLinkSlash }
                                "Unlink"
                            }
                        }
                    } else {
                        // 2. AdHoc mode actions: Edit & Publish expand; Catalog is a compact icon button
                        div { class: "flex-fill",
                            Button {
                                title: "Edit local material properties",
                                onclick: {
                                    let mat = current_material.clone();
                                    move |_| {
                                        editing_material.set(mat.clone());
                                        show_editor_dialog.set(true);
                                    }
                                },
                                Icon { icon: FaPencil }
                                "Edit"
                            }
                        }
                        div { class: "flex-fill",
                            Button {
                                title: "Publish this AdHoc material into the permanent catalog",
                                variant: ButtonVariant::Success,
                                onclick: on_publish_adhoc_to_catalog,
                                Icon { icon: FaCloudArrowUp }
                                "Publish"
                            }
                        }
                        // Compact icon-only button for picking from the catalog
                        div {
                            Button {
                                title: "Replace with an existing material from catalog",
                                variant: ButtonVariant::Outline,
                                onclick: move |_| show_catalog_dialog.set(true),
                                Icon { icon: FaBook }
                            }
                        }
                    }
                }
            }
        }

        // Catalog Selection Modal Dialog
        MaterialCatalog { open: show_catalog_dialog, on_select: on_catalog_select }

        // AdHoc Material Editor Modal Dialog
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
