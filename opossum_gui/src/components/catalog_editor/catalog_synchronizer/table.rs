use crate::components::primitives::{
    button::{Button, ButtonSize, ButtonVariant},
    scroll_area::ScrollArea,
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaBook, FaCheck, FaCloudArrowDown, FaCloudArrowUp, FaPlus},
};

use super::types::{AssetSyncRow, AssetSyncStatus};

/// Props for the catalog synchronizer table.
#[derive(Props, Clone, PartialEq)]
pub struct CatalogTableProps {
    /// Filtered list of asset sync rows to be rendered.
    pub rows: Vec<AssetSyncRow>,
    /// Total number of assets found in the loaded model (used for empty state distinction).
    pub total_model_assets: usize,
    /// Callback triggered when an outdated asset should be updated in the model.
    pub on_update_in_model: EventHandler<AssetSyncRow>,
    /// Callback triggered when a regular missing or newer asset should be imported/exported to the catalog.
    pub on_import_to_catalog: EventHandler<AssetSyncRow>,
    /// Callback triggered when an ad-hoc asset needs to be edited before catalog publishing.
    pub on_edit_adhoc: EventHandler<AssetSyncRow>,
}

/// Renders a scrollable comparison table showing model assets alongside their catalog status.
#[component]
pub fn CatalogTable(props: CatalogTableProps) -> Element {
    rsx! {
      ScrollArea { height: "24rem",
        div { class: "table-responsive",
          table { class: "table table-hover table-sm align-middle mb-0",
            thead { class: "table-light sticky-top",
              tr {
                th { "Asset Name" }
                th { "Category" }
                th { "Manufacturer" }
                th { "Model Version" }
                th { "Catalog Version" }
                th { "Used In Nodes" }
                th { "Status" }
                th { class: "text-end", "Actions" }
              }
            }
            tbody {
              for row in &props.rows {
                CatalogTableRow {
                  key: "{row.id}",
                  row: row.clone(),
                  on_update_in_model: props.on_update_in_model,
                  on_import_to_catalog: props.on_import_to_catalog,
                  on_edit_adhoc: props.on_edit_adhoc,
                }
              }
              if props.rows.is_empty() {
                tr {
                  td {
                    colspan: "8",
                    class: "text-center text-muted py-4",
                    if props.total_model_assets == 0 {
                      "No assets found in the currently loaded model."
                    } else {
                      "No assets match the active filter criteria."
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

/// Props for an individual row within the catalog synchronizer table.
#[derive(Props, Clone, PartialEq)]
pub struct CatalogTableRowProps {
    /// The asset synchronization data for this row.
    pub row: AssetSyncRow,
    /// Callback triggered when an outdated asset should be updated in the model.
    pub on_update_in_model: EventHandler<AssetSyncRow>,
    /// Callback triggered when a regular missing or newer asset should be imported/exported to the catalog.
    pub on_import_to_catalog: EventHandler<AssetSyncRow>,
    /// Callback triggered when an ad-hoc asset needs to be edited before catalog publishing.
    pub on_edit_adhoc: EventHandler<AssetSyncRow>,
}

/// Renders a single row representing an asset, its versions, node usages, and context actions.
#[component]
pub fn CatalogTableRow(props: CatalogTableRowProps) -> Element {
    let row = &props.row;

    // Concatenate node names for tooltip and truncated display
    let node_names = if row.used_by_nodes.is_empty() {
        "-".to_string()
    } else {
        row.used_by_nodes
            .iter()
            .map(|u| u.node_name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    rsx! {
      tr {
        // Asset Name
        td { class: "fw-bold", "{row.name}" }

        // Category Badge
        td {
          span { class: "badge bg-dark", "{row.category.label()}" }
        }

        // Manufacturer
        td { class: "text-muted", "{row.manufacturer.as_deref().unwrap_or(\"-\")}" }

        // Model Version Badge
        td {
          if row.model_version == 0 {
            span { class: "badge bg-warning text-dark", "v0 (Ad-Hoc)" }
          } else {
            span { class: "badge bg-secondary", "v{row.model_version}" }
          }
        }

        // Catalog Version Badge
        td {
          if let Some(v) = row.catalog_version {
            span { class: "badge bg-info text-dark", "v{v}" }
          } else {
            span { class: "text-muted", "-" }
          }
        }

        // Node Usages with truncation and full tooltip
        td {
          div {
            class: "small text-truncate",
            style: "max-width: 14rem;",
            title: "{node_names}",
            "{node_names}"
          }
        }

        // Status Badge
        td {
          match row.status {
              AssetSyncStatus::UpToDate => rsx! {
                span { class: "badge bg-success d-inline-flex align-items-center gap-1",
                  Icon { icon: FaCheck }
                  "Up to date"
                }
              },
              AssetSyncStatus::ModelOutdated => rsx! {
                span { class: "badge bg-warning text-dark d-inline-flex align-items-center gap-1",
                  Icon { icon: FaCloudArrowDown }
                  "Update available"
                }
              },
              AssetSyncStatus::MissingInCatalogRegular => rsx! {
                span { class: "badge bg-primary d-inline-flex align-items-center gap-1",
                  Icon { icon: FaCloudArrowUp }
                  "Missing in catalog"
                }
              },
              AssetSyncStatus::MissingInCatalogAdHoc => rsx! {
                span { class: "badge bg-secondary d-inline-flex align-items-center gap-1",
                  Icon { icon: FaBook }
                  "Ad-Hoc Draft"
                }
              },
              AssetSyncStatus::ModelNewer => rsx! {
                span { class: "badge bg-info text-dark", "Model newer" }
              },
          }
        }

        // Action Buttons
        td { class: "text-end",
          match row.status {
              AssetSyncStatus::ModelOutdated => {
                  let row_clone = row.clone();
                  rsx! {
                    Button {
                      title: "Update all node properties in the model to the newer catalog version",
                      variant: ButtonVariant::Primary,
                      size: ButtonSize::Sm,
                      onclick: move |_| props.on_update_in_model.call(row_clone.clone()),
                      Icon { icon: FaCloudArrowDown }
                      "Update in Model"
                    }
                  }
              }
              AssetSyncStatus::MissingInCatalogRegular => {
                  let row_clone = row.clone();
                  rsx! {
                    Button {
                      title: "Import this regular material into the local catalog",
                      variant: ButtonVariant::Success,
                      size: ButtonSize::Sm,
                      onclick: move |_| props.on_import_to_catalog.call(row_clone.clone()),
                      Icon { icon: FaPlus }
                      "Add to Catalog"
                    }
                  }
              }
              AssetSyncStatus::MissingInCatalogAdHoc => {
                  let row_clone = row.clone();
                  rsx! {
                    Button {
                      title: "Open in Material Editor to review and publish to catalog",
                      variant: ButtonVariant::Primary,
                      size: ButtonSize::Sm,
                      onclick: move |_| props.on_edit_adhoc.call(row_clone.clone()),
                      Icon { icon: FaPlus }
                      "Add to Catalog"
                    }
                  }
              }
              AssetSyncStatus::UpToDate => rsx! {
                span { class: "text-success small d-inline-flex align-items-center gap-1",
                  Icon { icon: FaCheck }
                  "In Sync"
                }
              },
              AssetSyncStatus::ModelNewer => {
                  let row_clone = row.clone();
                  rsx! {
                    Button {
                      title: "Export this newer version to the catalog",
                      variant: ButtonVariant::Outline,
                      size: ButtonSize::Sm,
                      onclick: move |_| props.on_import_to_catalog.call(row_clone.clone()),
                      Icon { icon: FaCloudArrowUp }
                      "Update Catalog Item"
                    }
                  }
              }
          }
        }
      }
    }
}
