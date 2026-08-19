use dioxus::prelude::*;
use opossum_core::{material::Material, refractive_index::RefrIndexSellmeier1};
use opossum_registry::{
    AssetIndex, AssetLoader,
    index::{IndexEntry, MaterialIndexData},
};
use uuid::Uuid;

/// Automatically seeds standard optical materials (like N-BK7) if the registry is empty.
pub fn seed_catalog_if_empty(loader: &AssetLoader) {
    let mut index = AssetIndex::<Material>::new();
    if let Ok(count) = index.build_from_loader(loader) {
        if count == 0 {
            log::info!("Registry is empty. Seeding initial N-BK7 catalog material...");

            // Create default N-BK7 draft with standard Sellmeier 1 coefficients
            let mut nbk7 = Material::new_draft(
                "N-BK7",
                Some("Schott".to_string()),
                Some("Primary crown glass for optical lenses and prisms".to_string()),
                RefrIndexSellmeier1::default().into(),
            );

            // Publish as version 1 to disk
            if let Err(e) = loader.publish(&mut nbk7) {
                log::error!("Failed to seed initial N-BK7 material: {}", e);
            }
        }
    }
}

/// Events emitted by the catalog component.
#[derive(Debug, Clone, PartialEq)]
pub enum MaterialCatalogEvent {
    /// Request to edit an existing material (passes its UUID).
    Edit(Uuid),
    /// Request to create a completely new material.
    CreateNew,
}

/// Properties for the `MaterialCatalog` component.
#[derive(Props, Clone, PartialEq)]
pub struct MaterialCatalogProps {
    /// Shared signal referencing the asset loader for disk access.
    pub loader: Signal<AssetLoader>,
    /// Reactive trigger to force a re-index from disk (e.g. after saving).
    #[props(default)]
    pub refresh_trigger: Signal<usize>,
    /// Handler for catalog actions (Edit, Create).
    pub on_action: EventHandler<MaterialCatalogEvent>,
}

#[component]
pub fn MaterialCatalog(props: MaterialCatalogProps) -> Element {
    // 1. In-memory index signal
    let mut index_sig = use_signal(|| {
        let loader = props.loader.read();
        seed_catalog_if_empty(&loader);

        let mut idx = AssetIndex::<Material>::new();
        let _ = idx.build_from_loader(&loader);
        idx
    });

    // 2. Watch refresh trigger to rebuild index when external changes occur
    use_effect(move || {
        let _ = props.refresh_trigger.read();
        let loader = props.loader.read();
        let mut idx = index_sig.write();
        let _ = idx.build_from_loader(&loader);
    });

    // 3. Local search & filter states
    let mut search_text = use_signal(String::new);
    let mut min_nd = use_signal(|| Option::<f64>::None);
    let mut max_nd = use_signal(|| Option::<f64>::None);

    // 4. Derived State: Filtered entries
    let filtered_entries = use_memo(move || {
        let text_query = search_text.read();
        let idx = index_sig.read();
        let trimmed_query = text_query.trim();

        // 1. Perform text search across Name, Manufacturer, and Description
        let mut results: Vec<IndexEntry<MaterialIndexData>> = if trimmed_query.is_empty() {
            idx.all_entries().into_iter().cloned().collect()
        } else {
            idx.search_text(trimmed_query)
                .into_iter()
                .cloned()
                .collect()
        };

        // 2. Apply refractive index (nd) filters if set
        if let Some(min) = *min_nd.read() {
            results.retain(|e| e.specific.nd.is_some_and(|nd| nd >= min));
        }
        if let Some(max) = *max_nd.read() {
            results.retain(|e| e.specific.nd.is_some_and(|nd| nd <= max));
        }

        results
    });

    rsx! {
      div { class: "card h-100",
        div { class: "card-header bg-dark text-white d-flex justify-content-between align-items-center",
          h5 { class: "mb-0", "Material Catalog" }
          button {
            class: "btn btn-sm btn-success",
            r#type: "button",
            onclick: move |_| props.on_action.call(MaterialCatalogEvent::CreateNew),
            "➕ New Material"
          }
        }
        div { class: "card-body overflow-auto",
          // Filter controls
          div { class: "row mb-4 align-items-end",
            div { class: "col-md-4",
              label { class: "form-label fw-bold small", "Search Name / Manufacturer" }
              input {
                class: "form-control form-control-sm",
                r#type: "text",
                placeholder: "e.g., N-BK7 or Schott",
                value: "{search_text}",
                oninput: move |e| search_text.set(e.value()),
              }
            }
            div { class: "col-md-3",
              label { class: "form-label fw-bold small", "Min Refractive Index (nd)" }
              input {
                class: "form-control form-control-sm",
                r#type: "number",
                step: "0.01",
                oninput: move |e| min_nd.set(e.value().parse().ok()),
              }
            }
            div { class: "col-md-3",
              label { class: "form-label fw-bold small", "Max Refractive Index (nd)" }
              input {
                class: "form-control form-control-sm",
                r#type: "number",
                step: "0.01",
                oninput: move |e| max_nd.set(e.value().parse().ok()),
              }
            }
            div { class: "col-md-2 text-end",
              button {
                class: "btn btn-sm btn-outline-secondary w-100",
                r#type: "button",
                onclick: move |_| {
                    let loader = props.loader.read();
                    let _ = index_sig.write().build_from_loader(&loader);
                },
                "🔄 Refresh"
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
                    td { "{entry.common.manufacturer.as_deref().unwrap_or(\"-\")}" }
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
                      button {
                        class: "btn btn-sm btn-outline-primary",
                        r#type: "button",
                        onclick: {
                            let id = entry.common.id;
                            move |_| props.on_action.call(MaterialCatalogEvent::Edit(id))
                        },
                        "Edit"
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
}
