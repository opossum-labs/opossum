use crate::components::{
    asset_editor::material_editor::{MaterialChangeEvent, MaterialEditor},
    primitives::{
        alert_dialog::{
            AlertDialog, AlertDialogAction, AlertDialogActions, AlertDialogCancel,
            AlertDialogDescription, AlertDialogTitle,
        },
        button::{Button, ButtonVariant},
        card::{Card, CardAction, CardContent, CardHeader, CardTitle},
    },
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaArrowsRotate, FaPencil, FaPlus, FaTrash},
};
use dioxus_primitives::alert_dialog::AlertDialogContent;
use opossum_core::{material::Material, refractive_index::RefrIndexSellmeier1};
use opossum_registry::{
    AssetIndex, AssetLoader,
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
#[derive(Debug, Clone, PartialEq)]
pub enum MaterialCatalogEvent {
    /// A new material (or new version of an existing one) has been published
    MaterialAdded,
    /// A material has been deleted
    MaterialDeleted,
}

#[component]
pub fn MaterialCatalog(
    open: Signal<bool>,
    /// Shared signal referencing the asset loader for disk access.
    loader: ReadSignal<AssetLoader>,
    /// Handler for catalog actions (Edit, Create).
    #[props(default)]
    on_action: EventHandler<MaterialCatalogEvent>,
) -> Element {
    let mut open_materialeditor = use_signal(|| false);
    let mut material_state = use_signal(|| Material::default());
    let mut index_sig = use_signal(|| {
        let mut idx = AssetIndex::<Material>::new();
        let _ = idx.build_from_loader(&loader.read());
        idx
    });
    let mut show_delete_dialog = use_signal(|| false);
    let mut pending_delete = use_signal(|| Option::<DeleteTarget>::None);

    let mut search_text = use_signal(String::new);
    let mut min_nd = use_signal(|| Option::<f64>::None);
    let mut max_nd = use_signal(|| Option::<f64>::None);

    let filtered_entries = use_memo(move || {
        let text_query = search_text.read();
        let idx = index_sig.read();
        let trimmed_query = text_query.trim();

        let mut results: Vec<IndexEntry<MaterialIndexData>> = if trimmed_query.is_empty() {
            idx.all_entries().into_iter().cloned().collect()
        } else {
            idx.search_text(trimmed_query)
                .into_iter()
                .cloned()
                .collect()
        };

        if let Some(min) = *min_nd.read() {
            results.retain(|e| e.specific.nd.is_some_and(|nd| nd >= min));
        }
        if let Some(max) = *max_nd.read() {
            results.retain(|e| e.specific.nd.is_some_and(|nd| nd <= max));
        }

        results.sort_by(|a, b| {
            a.common
                .name
                .to_lowercase()
                .cmp(&b.common.name.to_lowercase())
        });

        results
    });
    let on_material_changed = use_callback(move |e: MaterialChangeEvent| {
        info!("Material property modified: {e:?}");
        e.action.apply(&mut material_state.write());
    });
    let on_material_save = use_callback(move |()| {
        info!("Publishing material draft to registry...");

        // Dereference the WriteLock guard to pass `&mut Material` instead of `&mut WriteLock`
        match loader.read().publish(&mut *material_state.write()) {
            Ok(saved_path) => {
                info!("Successfully saved material to {:?}", saved_path);
                // Trigger index reload in the catalog
                let _ = index_sig.write().build_from_loader(&loader.read());
                on_action.call(MaterialCatalogEvent::MaterialAdded);
            }
            Err(e) => {
                log::error!("Failed to save material to registry: {e}");
            }
        }
    });
    let on_create_new = use_callback(move |_| {
        info!("Opening editor for a new material draft...");
        // Initialize fresh draft with default values
        material_state.set(Material::new_draft(
            "New Material",
            None,
            None,
            RefrIndexSellmeier1::default().into(),
        ));
        open_materialeditor.set(true);
    });
    let on_edit_material = use_callback(move |id| {
        info!("Loading material {id} for editing...");
        match loader.read().load::<Material>(id, None) {
            Ok(loaded_material) => {
                // Create an editable draft from the published material
                material_state.set(loaded_material.new_draft_from());
                open_materialeditor.set(true);
            }
            Err(err) => {
                log::error!("Failed to load material from registry: {err}");
            }
        }
        open_materialeditor.set(true);
    });
    let mut execute_delete = move |target: DeleteTarget| {
        match loader.read().delete_latest_version::<Material>(target.id) {
            Ok(Some(new_latest)) => {
                log::info!(
                    "Deleted version v{} of '{}'. Rolled back to v{}.",
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
        let _ = index_sig.write().build_from_loader(&loader.read());
    };
    rsx! {
      AlertDialog {
        open: open(),
        on_open_change: move |v: bool| open.set(v),
        max_width: "55rem".to_string(),
        AlertDialogDescription {
          Card {
            CardHeader {
              CardTitle { "Material Catalog" }
              CardAction {
                Button { onclick: on_create_new,
                  Icon { icon: FaPlus }
                  "New Material"
                }
              }
            }
            CardContent {
              // Filter controls
              div { class: "row mb-4 align-items-end",
                div { class: "col-md-4",
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
                    "Min Refractive Index (nd)"
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
                    "Max Refractive Index (nd)"
                  }
                  input {
                    class: "form-control form-control-sm",
                    r#type: "number",
                    step: "0.01",
                    oninput: move |e| max_nd.set(e.value().parse().ok()),
                  }
                }
                div { class: "col-md-2 text-end",
                  Button {
                    title: "Refresh index",
                    onclick: move |_| {
                        let _ = index_sig.write().build_from_loader(&loader.read());
                    },
                    Icon { icon: FaArrowsRotate }
                    "Refresh"
                  }
                }
              }

              // Results Table
              div { class: "table-responsive",
                table { class: "table table-hover table-sm align-middle",
                  thead { class: "table-light",
                    tr {
                      th { "Name" }
                      th { "Manufacturer" }
                      th { "Refractive Index (nd)" }
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
                        td { class: "text-end",
                          Button {
                            title: "Add new version of the material",
                            onclick: {
                                let id = entry.common.id;
                                move |_evt| on_edit_material.call(id)
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
                    if filtered_entries.read().is_empty() {
                      tr {
                        td {
                          colspan: 5,
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
        AlertDialogActions {
          AlertDialogAction { "Close" }
        }
      }
      MaterialEditor {
        open: open_materialeditor,
        material: material_state,
        readonly: false,
        on_change: on_material_changed,
        on_save: on_material_save,
      }
      // Deletion Confirmation Dialog
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
            AlertDialogCancel {
              on_click: move |_| {
                  show_delete_dialog.set(false);
              },
              "Cancel"
            }
            AlertDialogAction {
              on_click: move |_| {
                  if let Some(target) = pending_delete.read().clone() {
                      execute_delete(target);
                  }
                  show_delete_dialog.set(false);
              },
              "Delete Version"
            }
          }
        }
      }
    }
}
