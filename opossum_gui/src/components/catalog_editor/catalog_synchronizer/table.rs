use crate::components::primitives::{
    button::{Button, ButtonSize, ButtonVariant},
    scroll_area::ScrollArea,
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{
        FaBook, FaCheck, FaCloudArrowDown, FaCloudArrowUp, FaPlus, FaTriangleExclamation,
    },
};

use super::types::{AssetSyncRow, AssetSyncStatus, CatalogOrigin};

/// Props for the catalog synchronizer table.
#[derive(Props, Clone, PartialEq)]
pub struct CatalogTableProps {
    /// Filtered list of asset sync rows to be rendered.
    pub rows: Vec<AssetSyncRow>,
    /// Total number of assets found in the loaded model (used for empty state distinction).
    pub total_model_assets: usize,
    /// Callback triggered when an outdated asset should be updated in the model.
    pub on_update_in_model: EventHandler<AssetSyncRow>,
    /// Callback triggered when a missing regular asset should be imported into the user catalog.
    pub on_import_to_user_catalog: EventHandler<AssetSyncRow>,
    /// Callback triggered when an ad-hoc asset needs to be edited before publishing to the user catalog.
    pub on_edit_adhoc: EventHandler<AssetSyncRow>,
    /// Callback triggered when the user should pull the official catalog for a newer model version.
    pub on_sync_official_catalog: EventHandler<()>,
    /// Callback triggered when a user catalog conflict must be resolved (overwrite / rename / cancel).
    pub on_resolve_user_conflict: EventHandler<AssetSyncRow>,
}

/// Renders a scrollable comparison table showing model assets alongside their official/user catalog status.
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
                            th { "Source" }
                            th { "Manufacturer" }
                            th { "Model Ver." }
                            th { "Catalog Ver." }
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
                                on_import_to_user_catalog: props.on_import_to_user_catalog,
                                on_edit_adhoc: props.on_edit_adhoc,
                                on_sync_official_catalog: props.on_sync_official_catalog,
                                on_resolve_user_conflict: props.on_resolve_user_conflict,
                            }
                        }
                        if props.rows.is_empty() {
                            tr {
                                td {
                                    colspan: "9",
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
    /// Callback triggered when a missing regular asset should be imported into the user catalog.
    pub on_import_to_user_catalog: EventHandler<AssetSyncRow>,
    /// Callback triggered when an ad-hoc asset needs to be edited before publishing to the user catalog.
    pub on_edit_adhoc: EventHandler<AssetSyncRow>,
    /// Callback triggered when the user should pull the official catalog for a newer model version.
    pub on_sync_official_catalog: EventHandler<()>,
    /// Callback triggered when a user catalog conflict must be resolved (overwrite / rename / cancel).
    pub on_resolve_user_conflict: EventHandler<AssetSyncRow>,
}

/// Renders a single row representing an asset, its catalog origin, versions, usages, and actions.
#[component]
pub fn CatalogTableRow(props: CatalogTableRowProps) -> Element {
    let row = &props.row;

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

            // Catalog Origin Badge (Official / User / None)
            td {
                match row.origin {
                    CatalogOrigin::Official => rsx! {
                        span { class: "badge bg-primary", "Official" }
                    },
                    CatalogOrigin::User => rsx! {
                        span { class: "badge bg-info text-dark", "User" }
                    },
                    CatalogOrigin::None => rsx! {
                        span { class: "badge bg-light text-dark border", "Unlisted" }
                    },
                }
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
                    style: "max-width: 12rem;",
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
                    AssetSyncStatus::OfficialSyncRecommended => rsx! {
                        span { class: "badge bg-info text-dark d-inline-flex align-items-center gap-1",
                            Icon { icon: FaCloudArrowDown }
                            "Pull Official Catalog"
                        }
                    },
                    AssetSyncStatus::MissingInCatalogRegular => rsx! {
                        span { class: "badge bg-primary d-inline-flex align-items-center gap-1",
                            Icon { icon: FaCloudArrowUp }
                            "New for User Catalog"
                        }
                    },
                    AssetSyncStatus::MissingInCatalogAdHoc => rsx! {
                        span { class: "badge bg-secondary d-inline-flex align-items-center gap-1",
                            Icon { icon: FaBook }
                            "Ad-Hoc Draft"
                        }
                    },
                    AssetSyncStatus::UserConflict => rsx! {
                        span { class: "badge bg-danger d-inline-flex align-items-center gap-1",
                            Icon { icon: FaTriangleExclamation }
                            "User Conflict"
                        }
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
                    AssetSyncStatus::OfficialSyncRecommended => rsx! {
                        Button {
                            title: "Model has a higher version of this official material; sync the official catalog",
                            variant: ButtonVariant::Primary,
                            size: ButtonSize::Sm,
                            onclick: move |_| props.on_sync_official_catalog.call(()),
                            Icon { icon: FaCloudArrowDown }
                            "Sync Official"
                        }
                    },
                    AssetSyncStatus::MissingInCatalogRegular => {
                        let row_clone = row.clone();
                        rsx! {
                            Button {
                                title: "Import this material into the local user catalog",
                                variant: ButtonVariant::Success,
                                size: ButtonSize::Sm,
                                onclick: move |_| props.on_import_to_user_catalog.call(row_clone.clone()),
                                Icon { icon: FaPlus }
                                "Add to User Catalog"
                            }
                        }
                    }
                    AssetSyncStatus::MissingInCatalogAdHoc => {
                        let row_clone = row.clone();
                        rsx! {
                            Button {
                                title: "Open in Material Editor to review and publish to the user catalog",
                                variant: ButtonVariant::Primary,
                                size: ButtonSize::Sm,
                                onclick: move |_| props.on_edit_adhoc.call(row_clone.clone()),
                                Icon { icon: FaPlus }
                                "Add to User Catalog"
                            }
                        }
                    }
                    AssetSyncStatus::UserConflict => {
                        let row_clone = row.clone();
                        rsx! {
                            Button {
                                title: "Resolve conflict in user catalog (Overwrite, Save as New, or Cancel)",
                                variant: ButtonVariant::Outline,
                                size: ButtonSize::Sm,
                                onclick: move |_| props.on_resolve_user_conflict.call(row_clone.clone()),
                                Icon { icon: FaTriangleExclamation }
                                "Resolve Conflict"
                            }
                        }
                    }
                    AssetSyncStatus::UpToDate => rsx! {
                        span { class: "text-success small d-inline-flex align-items-center gap-1",
                            Icon { icon: FaCheck }
                            "In Sync"
                        }
                    },
                }
            }
        }
    }
}
