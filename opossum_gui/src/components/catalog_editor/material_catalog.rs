use crate::components::{
    asset_editor::material_editor::{MaterialChangeEvent, MaterialEditor},
    primitives::{
        alert_dialog::{
            AlertDialog, AlertDialogAction, AlertDialogActions, AlertDialogCancel,
            AlertDialogDescription, AlertDialogTitle,
        },
        button::{Button, ButtonVariant},
        card::{Card, CardAction, CardContent, CardHeader, CardTitle},
        scroll_area::ScrollArea,
    },
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaCheck, FaPencil, FaPlus, FaTrash},
};
use dioxus_primitives::alert_dialog::AlertDialogContent;
use opossum_core::{material::Material, refractive_index::RefrIndexSellmeier1};
use opossum_registry::{
    AssetRegistry,
    index::{IndexEntry, MaterialIndexData},
};
use uuid::Uuid;

/// Metadata of an asset queued for deletion confirmation.
#[derive(Debug, Clone, PartialEq)]
struct DeleteTarget {
    id: Uuid,
    name: String,
    latest_version: u32,
    total_versions_count: usize,
}

/// Events emitted by the catalog component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialCatalogEvent {
    /// A new material (or new version of an existing one) has been published.
    MaterialAdded,
    /// A material version has been deleted.
    MaterialDeleted,
}

/// Helper function to truncate long descriptions and append an ellipsis.
fn truncate_description(desc: Option<&str>, max_chars: usize) -> String {
    desc.map_or_else(
        || "-".to_string(),
        |text| {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                "-".to_string()
            } else if trimmed.chars().count() > max_chars {
                let truncated: String = trimmed.chars().take(max_chars).collect();
                format!("{truncated}...")
            } else {
                trimmed.to_string()
            }
        },
    )
}

#[component]
pub fn MaterialCatalog(
    /// Controls whether the catalog modal dialog is visible.
    open: Signal<bool>,
    /// Optional handler for selecting a material (activates Selector Mode).
    #[props(default)]
    on_select: Option<EventHandler<Material>>,
    /// Handler for catalog mutation events (Edit, Create, Delete).
    #[props(default)]
    on_action: EventHandler<MaterialCatalogEvent>,
) -> Element {
    // Shared registry facade accessed directly via Dioxus context
    let mut registry = use_context::<Signal<AssetRegistry<Material>>>();

    let is_select_mode = on_select.is_some();

    // Local UI states
    let mut open_materialeditor = use_signal(|| false);
    let mut material_state = use_signal(Material::default);
    let mut show_delete_dialog = use_signal(|| false);
    let mut pending_delete = use_signal(|| Option::<DeleteTarget>::None);

    // Search and filter inputs
    let mut search_text = use_signal(String::new);
    let mut min_nd = use_signal(|| Option::<f64>::None);
    let mut max_nd = use_signal(|| Option::<f64>::None);

    // Reactive filter pipeline reading from the in-memory registry index
    let filtered_entries = use_memo(move || {
        let text_query = search_text.read();
        let reg = registry.read();
        let trimmed_query = text_query.trim();

        // 1. Filter entries via registry index
        let mut results: Vec<IndexEntry<MaterialIndexData>> = if trimmed_query.is_empty() {
            reg.all_entries().into_iter().cloned().collect()
        } else {
            reg.search_text(trimmed_query)
                .into_iter()
                .cloned()
                .collect()
        };

        // 2. Apply refractive index (n_d) bounds
        if let Some(min) = *min_nd.read() {
            results.retain(|e| e.specific.nd.is_some_and(|nd| nd >= min));
        }
        if let Some(max) = *max_nd.read() {
            results.retain(|e| e.specific.nd.is_some_and(|nd| nd <= max));
        }

        // 3. Sort entries alphabetically by name (case-insensitive)
        results.sort_by_cached_key(|entry| entry.common.name.to_lowercase());
        results
    });

    // Callback: Handle inline edits from MaterialEditor
    let on_material_changed = use_callback(move |e: MaterialChangeEvent| {
        e.action.apply(&mut material_state.write());
    });

    // Callback: Publish draft to disk (automatically synchronizes in-memory index)
    let on_material_save =
        use_callback(
            move |()| match registry.write().publish(&mut *material_state.write()) {
                Ok(saved_path) => {
                    info!("Successfully published material to {:?}", saved_path);
                    on_action.call(MaterialCatalogEvent::MaterialAdded);
                }
                Err(e) => {
                    log::error!("Failed to publish material to registry: {e}");
                }
            },
        );

    // Callback: Initialize a fresh draft
    let on_create_new = use_callback(move |_| {
        material_state.set(Material::new_draft(
            "New Material",
            None,
            None,
            RefrIndexSellmeier1::default().into(),
        ));
        open_materialeditor.set(true);
    });

    // Callback: Load an existing asset and prepare a new draft version
    let on_edit_material = use_callback(move |id: Uuid| match registry.read().load(id, None) {
        Ok(loaded_material) => {
            material_state.set(loaded_material.new_draft_from());
            open_materialeditor.set(true);
        }
        Err(err) => {
            log::error!("Failed to load material from registry: {err}");
        }
    });

    // Callback: Delete latest version and update the in-memory cache
    let handle_execute_delete = use_callback(move |target: DeleteTarget| {
        match registry.write().delete_latest_version(target.id) {
            Ok(Some(new_latest)) => {
                log::info!(
                    "Deleted version v{} of '{}'. Reverted to v{}.",
                    target.latest_version,
                    target.name,
                    new_latest
                );
                on_action.call(MaterialCatalogEvent::MaterialDeleted);
            }
            Ok(None) => {
                log::info!(
                    "Deleted final version of '{}'. Material removed from registry.",
                    target.name
                );
                on_action.call(MaterialCatalogEvent::MaterialDeleted);
            }
            Err(e) => {
                log::error!("Failed to delete material version: {e}");
            }
        }
    });

    // Callback: Confirm deletion modal
    let handle_confirm_delete = use_callback(move |_| {
        if let Some(target) = pending_delete.read().clone() {
            handle_execute_delete.call(target);
        }
        show_delete_dialog.set(false);
    });

    // Callback: Cancel deletion modal
    let handle_cancel_delete = use_callback(move |_| {
        show_delete_dialog.set(false);
    });

    rsx! {
      // 1. Main Catalog Modal Dialog
      AlertDialog {
        open: open(),
        on_open_change: move |v: bool| open.set(v),
        max_width: "60rem".to_string(),
        AlertDialogDescription {
          Card {
            CardHeader {
              CardTitle {
                if is_select_mode {
                  "Select Material"
                } else {
                  "Material Catalog"
                }
              }
              if !is_select_mode {
                CardAction {
                  Button {
                    variant: ButtonVariant::Success,
                    onclick: on_create_new,
                    Icon { icon: FaPlus }
                    "New Material"
                  }
                }
              }
            }
            CardContent {
              // Search & Filter Toolbar (Optimized 3-column layout)
              div { class: "row mb-4 align-items-end",
                div { class: "col-md-6",
                  label { class: "form-label fw-bold small",
                    "Search Name / Manufacturer"
                  }
                  input {
                    class: "form-control form-control-sm",
                    r#type: "text",
                    placeholder: "e.g., N-BK7 or Schott",
                    value: "{search_text}",
                    oninput: move |e| search_text.set(e.value()),
                  }
                }
                div { class: "col-md-3",
                  label { class: "form-label fw-bold small",
                    "Min "
                    span {
                      "n"
                      sub { "d" }
                    }
                  }
                  input {
                    class: "form-control form-control-sm",
                    r#type: "number",
                    step: "0.01",
                    oninput: move |e| min_nd.set(e.value().parse().ok()),
                  }
                }
                div { class: "col-md-3",
                  label { class: "form-label fw-bold small",
                    "Max "
                    span {
                      "n"
                      sub { "d" }
                    }
                  }
                  input {
                    class: "form-control form-control-sm",
                    r#type: "number",
                    step: "0.01",
                    oninput: move |e| max_nd.set(e.value().parse().ok()),
                  }
                }
              }

              // Results Table View
              ScrollArea { height: "25rem",
                div { class: "table-responsive",
                  table { class: "table table-hover table-sm align-middle mb-0",
                    thead { class: "table-light sticky-top",
                      tr {
                        th { "Name" }
                        th { "Manufacturer" }
                        th { "Description" }
                        th {
                          "Refractive Index ("
                          span {
                            "n"
                            sub { "d" }
                          }
                          ")"
                        }
                        th { "Latest Version" }
                        th { class: "text-end", "Actions" }
                      }
                    }
                    tbody {
                      for entry in filtered_entries.read().iter() {
                        tr { key: "{entry.common.id}",
                          td { class: "fw-bold", "{entry.common.name}" }
                          td {
                            "{entry.common.manufacturer.as_deref().unwrap_or(\"-\")}"
                          }
                          td {
                            title: "{entry.common.description.as_deref().unwrap_or_default()}",
                            class: "text-muted small",
                            "{truncate_description(entry.common.description.as_deref(), 35)}"
                          }
                          td {
                            if let Some(nd) = entry.specific.nd {
                              "{nd:.4}"
                            } else {
                              "-"
                            }
                          }
                          td {
                            span { class: "badge bg-secondary",
                              "v{entry.common.latest_version}"
                            }
                          }
                          td { class: "text-end d-flex justify-content-end gap-1",
                            // In Select Mode: Render only the Select button
                            if let Some(select_handler) = on_select {
                              Button {
                                title: "Select this material for the current node",
                                variant: ButtonVariant::Primary,
                                onclick: {
                                    let id = entry.common.id;
                                    let version = entry.common.latest_version;
                                    move |_| {
                                        if let Ok(loaded) = registry.read().load(id, Some(version)) {
                                            select_handler.call(loaded);
                                            open.set(false);
                                        }
                                    }
                                },
                                Icon { icon: FaCheck }
                                "Select"
                              }
                            } else {
                              // In Catalog Management Mode: Render Edit & Delete actions
                              Button {
                                title: "Add new version of the material",
                                onclick: {
                                    let id = entry.common.id;
                                    move |_| on_edit_material.call(id)
                                },
                                Icon { icon: FaPencil }
                              }
                              Button {
                                title: "Delete latest version",
                                variant: ButtonVariant::Destructive,
                                onclick: {
                                    let target = DeleteTarget {
                                        id: entry.common.id,
                                        name: entry.common.name.clone(),
                                        latest_version: entry.common.latest_version,
                                        total_versions_count: entry.common.available_versions.len(),
                                    };
                                    move |_| {
                                        pending_delete.set(Some(target.clone()));
                                        show_delete_dialog.set(true);
                                    }
                                },
                                Icon { icon: FaTrash }
                              }
                            }
                          }
                        }
                      }
                      if filtered_entries.read().is_empty() {
                        tr {
                          td {
                            colspan: 6,
                            class: "text-center text-muted py-4",
                            "No materials found matching the current filters."
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
        AlertDialogActions {
          AlertDialogAction { "Close" }
        }
      }

      // 2. Material Editor Dialog (only mounted when in Management Mode)
      if !is_select_mode {
        MaterialEditor {
          open: open_materialeditor,
          material: material_state,
          readonly: false,
          on_change: on_material_changed,
          on_save: on_material_save,
        }
      }

      // 3. Deletion Confirmation Dialog
      AlertDialog {
        open: show_delete_dialog(),
        on_open_change: move |open: bool| {
            show_delete_dialog.set(open);
        },
        AlertDialogContent {
          AlertDialogTitle { "Delete Material Version" }
          AlertDialogDescription {
            if let Some(target) = pending_delete.read().as_ref() {
              if target.total_versions_count <= 1 {
                p { class: "text-danger fw-bold mb-1",
                  "⚠️ Warning: This is the only version of '{target.name}'."
                }
                p { class: "mb-0",
                  "Deleting version v{target.latest_version} will permanently remove the entire material from the catalog. Are you sure you want to proceed?"
                }
              } else {
                p { class: "mb-0",
                  "Are you sure you want to delete the latest version (v{target.latest_version}) of '{target.name}'? The material will revert to v{target.latest_version - 1}."
                }
              }
            }
          }
          AlertDialogActions {
            AlertDialogCancel { on_click: handle_cancel_delete, "Cancel" }
            AlertDialogAction { on_click: handle_confirm_delete, "Delete Version" }
          }
        }
      }
    }
}
