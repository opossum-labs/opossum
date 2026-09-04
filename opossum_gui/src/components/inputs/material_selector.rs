use crate::components::{
    asset_editor::material_editor::{MaterialChangeEvent, MaterialEditor},
    catalog_editor::MaterialCatalog,
    primitives::button::{Button, ButtonSize, ButtonVariant},
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaBook, FaCloudArrowUp, FaLinkSlash, FaPencil},
};
use opossum_core::material::Material;
use opossum_registry::AssetRegistry;

/// Reusable component for selecting, editing, unlinking, and publishing materials.
#[component]
pub fn MaterialSelector(
    /// Optional label displayed above the selector.
    #[props(default)]
    label: Option<String>,
    /// The material to display and modify.
    material: Material,
    /// Event emitted when the material is updated, replaced, unlinked, or published.
    on_change: EventHandler<Material>,
    /// Flag to disable editing actions.
    #[props(default = false)]
    readonly: bool,
) -> Element {
    // Access global asset registry from Dioxus context
    let mut registry = use_context::<Signal<AssetRegistry<Material>>>();

    // Extract display information from the material
    let is_catalog = material.version() > 0;
    let material_name = material.name().to_string();
    let current_version = material.version();
    // Floating-label text; empty when no label was supplied.
    let label_text = label.unwrap_or_default();

    // Modal dialog visibility signals
    let mut show_catalog_dialog = use_signal(|| false);
    let mut show_editor_dialog = use_signal(|| false);

    // Local buffer for the inline AdHoc material editor
    let mut editing_material = use_signal(|| material.clone());

    // Callback: Material chosen from the catalog dialog
    let on_catalog_select = {
        let on_change = on_change;
        use_callback(move |selected_mat: Material| {
            info!("Selected material from catalog: {}", selected_mat.name());
            on_change.call(selected_mat);
        })
    };

    // Callback: Detach catalog material into an independent AdHoc draft
    let on_unlink_to_adhoc = {
        let on_change = on_change;
        let material = material.clone();
        use_callback(move |_| {
            info!("Unlinking material '{}' to AdHoc draft...", material.name());
            let adhoc_copy = material.clone_as_adhoc();
            on_change.call(adhoc_copy);
        })
    };

    // Callback: Publish AdHoc material into the catalog registry
    let on_publish_adhoc_to_catalog = {
        let on_change = on_change;
        let mut material = material.clone();
        use_callback(move |_| {
            info!(
                "Publishing AdHoc material '{}' to registry...",
                material.name()
            );
            match registry.write().publish(&mut material) {
                Ok(_) => on_change.call(material.clone()),
                Err(err) => log::error!("Failed to publish material: {err}"),
            }
        })
    };

    // Callbacks for inline AdHoc MaterialEditor dialog
    let on_inline_editor_change = use_callback(move |evt: MaterialChangeEvent| {
        evt.action.apply(&mut editing_material.write());
    });

    let on_inline_editor_save = {
        let on_change = on_change;
        use_callback(move |()| {
            let updated = editing_material.read().clone();
            on_change.call(updated);
            show_editor_dialog.set(false);
        })
    };

    rsx! {
        // Compact material display matching the lens node's material-property editor:
        // name + version/AdHoc badge + icon-only action buttons inside a floating-label field.
        div { class: "form-floating border-start",
            // `bg-dark` is set explicitly: like `LabeledSelect`, this `.form-control` lives outside an
            // `.accordion-body` (the analyzer sidebar), so it misses the accordion-scoped dark rule in
            // `mdb_accordion.css` and would otherwise fall back to Bootstrap's white default.
            div { class: "form-control form-control-sm material-prop-display bg-dark",
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
                            title: "Detach from catalog (create an independent local copy)",
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
                                move |_| {
                                    editing_material.set(material.clone());
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
                            title: "Replace with an existing material from catalog",
                            class: "material-btn",
                            onclick: move |_| show_catalog_dialog.set(true),
                            Icon { icon: FaBook }
                        }
                    }
                }
            }
            label { class: "form-label text-secondary", "{label_text}" }
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
