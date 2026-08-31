use crate::components::{
    asset_editor::material_editor::{MaterialChangeEvent, MaterialEditor},
    catalog_editor::MaterialCatalog,
    primitives::button::{Button, ButtonVariant},
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
      div { class: "mb-3",
        // Optional Label Header
        if let Some(lbl) = label {
          label { class: "form-label fw-bold small text-capitalize mb-1", "{lbl}" }
        }

        // Row 1: Material name and status badge
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
              // Catalog Mode: Change or Unlink
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
              // AdHoc Mode: Edit inline, Publish, or replace from Catalog
              div { class: "flex-fill",
                Button {
                  title: "Edit local material properties",
                  onclick: {
                      move |_| {
                          editing_material.set(material.clone());
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
