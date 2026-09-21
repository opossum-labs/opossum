use crate::components::primitives::button::{Button, ButtonSize, ButtonVariant};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaCloudArrowDown, FaCloudArrowUp},
};

use super::types::{AssetCategory, StatusFilter};

/// Props for the catalog synchronizer filter and action toolbar.
#[derive(Props, Clone, PartialEq)]
pub struct CatalogToolbarProps {
    /// Two-way binding signal for the selected asset category.
    pub selected_category: Signal<Option<AssetCategory>>,
    /// Two-way binding signal for the selected status filter.
    pub selected_status_filter: Signal<StatusFilter>,
    /// Two-way binding signal for the text search query.
    pub search_query: Signal<String>,
    /// Total number of assets found in the current model.
    pub total_count: usize,
    /// Number of assets that are outdated compared to the catalog.
    pub outdated_count: usize,
    /// Number of regular assets missing in the catalog.
    pub missing_regular_count: usize,
    /// Number of ad-hoc draft assets in the model.
    pub adhoc_count: usize,
    /// Callback triggered when the user initiates a batch update of outdated assets.
    pub on_update_all: EventHandler<()>,
    /// Callback triggered when the user initiates a batch import of missing regular assets.
    pub on_import_all: EventHandler<()>,
}

/// Renders the toolbar containing category pills, status filter dropdown, search input, and batch buttons.
#[component]
pub fn CatalogToolbar(mut props: CatalogToolbarProps) -> Element {
    let needs_action_count = props.outdated_count + props.missing_regular_count + props.adhoc_count;
    let missing_total_count = props.missing_regular_count + props.adhoc_count;

    rsx! {
      div { class: "row mb-3 g-2 align-items-center",
        // Category selector group
        div { class: "col-auto",
          div { class: "btn-group btn-group-sm", role: "group",
            button {
              r#type: "button",
              class: if (props.selected_category)().is_none() { "btn btn-primary" } else { "btn btn-outline-secondary" },
              onclick: move |_| props.selected_category.set(None),
              "All Assets ({props.total_count})"
            }
            button {
              r#type: "button",
              class: if (props.selected_category)() == Some(AssetCategory::Material) { "btn btn-primary" } else { "btn btn-outline-secondary" },
              onclick: move |_| props.selected_category.set(Some(AssetCategory::Material)),
              "Materials ({props.total_count})"
            }
            button {
              r#type: "button",
              class: "btn btn-outline-secondary disabled",
              title: "Coatings catalog synchronization coming soon",
              "Coatings (0)"
            }
          }
        }

        // Status filter dropdown
        div { class: "col-auto",
          select {
            class: "form-select form-select-sm",
            value: match (props.selected_status_filter)() {
                StatusFilter::All => "all",
                StatusFilter::NeedsAction => "needs_action",
                StatusFilter::Outdated => "outdated",
                StatusFilter::Missing => "missing",
                StatusFilter::AdHocOnly => "adhoc",
                StatusFilter::UpToDate => "uptodate",
            },
            onchange: move |e| {
                let val = match e.value().as_str() {
                    "needs_action" => StatusFilter::NeedsAction,
                    "outdated" => StatusFilter::Outdated,
                    "missing" => StatusFilter::Missing,
                    "adhoc" => StatusFilter::AdHocOnly,
                    "uptodate" => StatusFilter::UpToDate,
                    _ => StatusFilter::All,
                };
                props.selected_status_filter.set(val);
            },
            option { value: "all", "All Statuses" }
            option { value: "needs_action", "Needs Action ({needs_action_count})" }
            option { value: "outdated", "Outdated in Model ({props.outdated_count})" }
            option { value: "missing", "Missing in Catalog ({missing_total_count})" }
            option { value: "adhoc", "Ad-Hoc Drafts ({props.adhoc_count})" }
            option { value: "uptodate", "Up to date" }
          }
        }

        // Search input field
        div { class: "col",
          input {
            class: "form-control form-control-sm",
            r#type: "text",
            placeholder: "Search by asset name or vendor...",
            value: "{props.search_query}",
            oninput: move |e| props.search_query.set(e.value()),
          }
        }

        // Batch action buttons
        div { class: "col-auto d-flex gap-2",
          if props.outdated_count > 0 {
            Button {
              variant: ButtonVariant::Primary,
              size: ButtonSize::Sm,
              title: "Update all outdated materials to the latest catalog version",
              onclick: move |_| props.on_update_all.call(()),
              Icon { icon: FaCloudArrowDown }
              "Update All ({props.outdated_count})"
            }
          }
          if props.missing_regular_count > 0 {
            Button {
              variant: ButtonVariant::Success,
              size: ButtonSize::Sm,
              title: "Import all missing regular materials into the local catalog",
              onclick: move |_| props.on_import_all.call(()),
              Icon { icon: FaCloudArrowUp }
              "Import All ({props.missing_regular_count})"
            }
          }
        }
      }
    }
}
