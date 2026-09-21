use crate::{
    APP_CONFIG,
    api::{get_document, update_node_property},
    components::{
        asset_editor::material_editor::{MaterialChangeEvent, MaterialEditor},
        primitives::{
            alert_dialog::{AlertDialog, AlertDialogActions, AlertDialogCancel, AlertDialogDescription},
            button::{Button, ButtonSize, ButtonVariant},
            card::{Card, CardAction, CardContent, CardHeader, CardTitle},
            scroll_area::ScrollArea,
        },
    },
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{
        FaArrowsRotate, FaBook, FaCheck, FaCircleExclamation, FaCloudArrowDown,
        FaCloudArrowUp, FaCodeBranch, FaPlus,
    },
};
use opossum_core::{
    material::Material,
    opm_document::OpmDocument,
    properties::{Proptype, proptype::AssetRef},
    refractive_index::RefrIndexSellmeier1,
};
use opossum_registry::{AssetRegistry, asset::RegisterableAsset};
#[cfg(not(target_arch = "wasm32"))]
use opossum_registry::RegistrySync;
use std::collections::HashMap;
use uuid::Uuid;

/// Represents an asset category for catalog synchronization.
/// Designed for future extensions (Coatings, Optical Components, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AssetCategory {
    Material,
    Coating,
}

impl AssetCategory {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Material => "Material",
            Self::Coating => "Coating",
        }
    }
}

/// Comparison status between an asset in the model and the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetSyncStatus {
    /// Model and catalog share the exact same version (v >= 1).
    UpToDate,
    /// Catalog contains a newer version than the model.
    ModelOutdated,
    /// Regular asset (v >= 1) exists in the model but is missing from the catalog.
    MissingInCatalogRegular,
    /// Ad-Hoc draft asset (v == 0) exists in the model and is not yet in the catalog.
    MissingInCatalogAdHoc,
    /// Model version is higher than the catalog's latest version.
    ModelNewer,
}

/// Identifies a node and property that uses an asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeUsageInfo {
    pub node_id: Uuid,
    pub node_name: String,
    pub property_key: String,
}

/// A row representing an asset found in the model compared to the catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct AssetSyncRow {
    pub id: Uuid,
    pub name: String,
    pub category: AssetCategory,
    pub manufacturer: Option<String>,
    pub model_version: u32,
    pub catalog_version: Option<u32>,
    pub status: AssetSyncStatus,
    pub used_by_nodes: Vec<NodeUsageInfo>,
    pub material: Option<Material>,
}


/// Filter options for the comparison table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    All,
    NeedsAction,
    Outdated,
    Missing,
    AdHocOnly,
    UpToDate,
}

/// Determines the synchronization status between an asset version in the model and the catalog.
#[must_use]
pub const fn determine_sync_status(
    model_version: u32,
    catalog_version: Option<u32>,
) -> AssetSyncStatus {
    match catalog_version {
        Some(latest_cat_v) => {
            if latest_cat_v > model_version {
                AssetSyncStatus::ModelOutdated
            } else if latest_cat_v == model_version && model_version >= 1 {
                AssetSyncStatus::UpToDate
            } else if model_version > latest_cat_v {
                AssetSyncStatus::ModelNewer
            } else {
                // model_version == 0 (Ad-Hoc) but catalog has a version
                AssetSyncStatus::ModelOutdated
            }
        }
        None => {
            if model_version == 0 {
                AssetSyncStatus::MissingInCatalogAdHoc
            } else {
                AssetSyncStatus::MissingInCatalogRegular
            }
        }
    }
}

/// Tests whether a sync row matches the given search query and filter criteria.
#[must_use]
pub fn matches_filter(
    row: &AssetSyncRow,
    query: &str,
    category: Option<AssetCategory>,
    status_filter: StatusFilter,
) -> bool {
    if let Some(cat) = category
        && row.category != cat
    {
        return false;
    }

    match status_filter {
        StatusFilter::All => {}
        StatusFilter::NeedsAction => {
            if row.status == AssetSyncStatus::UpToDate {
                return false;
            }
        }
        StatusFilter::Outdated => {
            if row.status != AssetSyncStatus::ModelOutdated {
                return false;
            }
        }
        StatusFilter::Missing => {
            if row.status != AssetSyncStatus::MissingInCatalogRegular
                && row.status != AssetSyncStatus::MissingInCatalogAdHoc
            {
                return false;
            }
        }
        StatusFilter::AdHocOnly => {
            if row.status != AssetSyncStatus::MissingInCatalogAdHoc {
                return false;
            }
        }
        StatusFilter::UpToDate => {
            if row.status != AssetSyncStatus::UpToDate {
                return false;
            }
        }
    }

    let trimmed = query.trim();
    if !trimmed.is_empty() {
        let q = trimmed.to_lowercase();
        let name_match = row.name.to_lowercase().contains(&q);
        let mfr_match = row
            .manufacturer
            .as_deref()
            .is_some_and(|m| m.to_lowercase().contains(&q));
        if !name_match && !mfr_match {
            return false;
        }
    }

    true
}

#[component]
pub fn CatalogSynchronizer(
    /// Controls whether the synchronizer modal dialog is visible.
    open: Signal<bool>,
) -> Element {
    let registry = use_context::<Signal<AssetRegistry<Material>>>();

    // Local UI state
    let mut rows = use_signal(Vec::<AssetSyncRow>::new);
    let mut is_scanning = use_signal(|| false);
    let mut error_message = use_signal(|| Option::<String>::None);
    let mut status_message = use_signal(|| Option::<String>::None);

    // Git sync state
    let mut is_git_syncing = use_signal(|| false);
    let mut git_sync_result = use_signal(|| Option::<Result<String, String>>::None);

    // Filter and search state
    let mut search_query = use_signal(String::new);
    let mut selected_category = use_signal(|| Option::<AssetCategory>::None);
    let mut selected_status_filter = use_signal(|| StatusFilter::All);

    // Ad-Hoc material editor state
    let mut open_material_editor = use_signal(|| false);
    let mut editing_material = use_signal(|| {
        Material::new_draft(
            "New Material",
            None,
            None,
            RefrIndexSellmeier1::default().into(),
        )
    });
    let mut editing_usages = use_signal(Vec::<NodeUsageInfo>::new);

    // Helper: Scans the currently loaded model and compares it with the catalog index
    let scan_model = move || {
        spawn(async move {
            is_scanning.set(true);
            error_message.set(None);

                let opm_string_res = get_document().await;
                match opm_string_res {
                    Ok(opm_str) => {
                        match OpmDocument::from_string(&opm_str) {
                            Ok(document) => {
                                let mut materials_map: HashMap<
                                    Uuid,
                                    (Material, Vec<NodeUsageInfo>),
                                > = HashMap::new();

                                // Traverse all nodes in scenery
                                if let Ok(all_nodes) =
                                    document.scenery().collect_all_nodes_recursive()
                                {
                                    for node_ref in all_nodes {
                                        let node_id = node_ref.node_attr().uuid();
                                        let node_name = node_ref.node_attr().name().to_string();

                                        for (prop_name, prop) in node_ref.node_attr().properties() {
                                            if let Proptype::Material(AssetRef::Inline(mat)) =
                                                prop.prop()
                                            {
                                                let entry = materials_map
                                                    .entry(mat.id())
                                                    .or_insert_with(|| (mat.clone(), Vec::new()));
                                                entry.1.push(NodeUsageInfo {
                                                    node_id,
                                                    node_name: node_name.clone(),
                                                    property_key: prop_name.clone(),
                                                });
                                            }
                                        }
                                    }
                                }

                                // Compare against catalog index
                                let reg = registry.read();
                                let mut computed_rows = Vec::new();

                                for (mat_id, (mat, usages)) in materials_map {
                                    let cat_entry = reg.index().get(&mat_id);
                                    let cat_version =
                                        cat_entry.map(|e| e.common.latest_version);

                                    let status = determine_sync_status(mat.version(), cat_version);

                                    computed_rows.push(AssetSyncRow {
                                        id: mat_id,
                                        name: mat.name().to_string(),
                                        category: AssetCategory::Material,
                                        manufacturer: mat.manufacturer().map(str::to_string),
                                        model_version: mat.version(),
                                        catalog_version: cat_version,
                                        status,
                                        used_by_nodes: usages,
                                        material: Some(mat),
                                    });
                                }

                                computed_rows.sort_by(|a, b| a.name.cmp(&b.name));
                                rows.set(computed_rows);
                            }
                            Err(e) => {
                                error_message
                                    .set(Some(format!("Failed to parse document: {e}")));
                            }
                        }
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Failed to fetch document: {e}")));
                    }
                }
                is_scanning.set(false);
            });
    };

    // Auto-scan whenever the dialog is opened
    use_effect(move || {
        if open() {
            scan_model();
        }
    });

    // Action: Update a single outdated material in the model to the latest catalog version
    let update_material_in_model = move |row: AssetSyncRow| {
        spawn(async move {
            let latest_mat = match registry.read().load(row.id, None) {
                Ok(m) => m,
                Err(e) => {
                    error_message
                        .set(Some(format!("Failed to load material from catalog: {e}")));
                    return;
                }
            };

            let mut update_errors = Vec::new();
            for usage in &row.used_by_nodes {
                let prop = Proptype::Material(AssetRef::Inline(latest_mat.clone()));
                if let Err(e) =
                    update_node_property(usage.node_id, (usage.property_key.clone(), prop))
                        .await
                {
                    update_errors.push(format!("{}: {e}", usage.node_name));
                }
            }

            if update_errors.is_empty() {
                status_message.set(Some(format!(
                    "Updated '{}' to v{} in {} node(s).",
                    row.name,
                    latest_mat.version(),
                    row.used_by_nodes.len()
                )));
                *crate::NODE_DETAILS_REFRESH.write() += 1;
                scan_model();
            } else {
                error_message.set(Some(format!(
                    "Errors updating nodes: {}",
                    update_errors.join("; ")
                )));
            }
        });
    };

    // Action: Import regular material (v >= 1) into catalog
    let import_regular_to_catalog = use_callback(move |row: AssetSyncRow| {
        let mut registry = registry;
        if let Some(mut mat) = row.material {
            match registry.write().publish(&mut mat) {
                Ok(path) => {
                    status_message.set(Some(format!(
                        "Imported '{}' (v{}) into catalog at {}.",
                        row.name,
                        mat.version(),
                        path.display()
                    )));
                    scan_model();
                }
                Err(e) => {
                    error_message
                        .set(Some(format!("Failed to publish to catalog: {e}")));
                }
            }
        }
    });

    // Callback: Handle property edits from MaterialEditor
    let on_material_changed = use_callback(move |e: MaterialChangeEvent| {
        e.action.apply(&mut editing_material.write());
    });

    // Callback: Publish edited Ad-Hoc material to catalog and update referencing model nodes
    let on_material_save = use_callback(move |()| {
        let mut registry = registry;
        let mut mat = editing_material.read().clone();
        match registry.write().publish(&mut mat) {
            Ok(path) => {
                let published_version = mat.version();
                let published_name = mat.name().to_string();
                let usages = editing_usages.read().clone();

                spawn(async move {
                    for usage in &usages {
                        let prop = Proptype::Material(AssetRef::Inline(mat.clone()));
                        let _ = update_node_property(
                            usage.node_id,
                            (usage.property_key.clone(), prop),
                        )
                        .await;
                    }
                    *crate::NODE_DETAILS_REFRESH.write() += 1;
                    status_message.set(Some(format!(
                        "Published '{published_name}' as v{published_version} to catalog ({}) and updated {} model node(s).",
                        path.display(),
                        usages.len()
                    )));
                    scan_model();
                });
            }
            Err(e) => {
                error_message.set(Some(format!("Failed to publish material to catalog: {e}")));
            }
        }
    });

    // Action: Batch update all outdated materials in the model
    let update_all_outdated = move |_| {
        let current_rows = rows.read().clone();
        spawn(async move {
            let outdated_rows: Vec<_> = current_rows
                .into_iter()
                .filter(|r| r.status == AssetSyncStatus::ModelOutdated)
                .collect();

            if outdated_rows.is_empty() {
                return;
            }

            let mut updated_count = 0;
            for row in outdated_rows {
                let latest_mat = registry.read().load(row.id, None).ok();
                if let Some(latest_mat) = latest_mat {
                    for usage in &row.used_by_nodes {
                        let prop = Proptype::Material(AssetRef::Inline(latest_mat.clone()));
                        let _ = update_node_property(
                            usage.node_id,
                            (usage.property_key.clone(), prop),
                        )
                        .await;
                    }
                    updated_count += 1;
                }
            }

            *crate::NODE_DETAILS_REFRESH.write() += 1;
            status_message.set(Some(format!(
                "Batch updated {updated_count} material(s) to their latest catalog versions."
            )));
            scan_model();
        });
    };

    // Action: Batch import all missing regular materials into catalog
    let import_all_missing = move |_| {
        let mut registry = registry;
        let mut imported = 0;
        let missing_rows: Vec<_> = rows
            .read()
            .iter()
            .filter(|r| r.status == AssetSyncStatus::MissingInCatalogRegular)
            .cloned()
            .collect();

        for row in missing_rows {
            if let Some(mut mat) = row.material
                && registry.write().publish(&mut mat).is_ok()
            {
                imported += 1;
            }
        }

        if imported > 0 {
            status_message.set(Some(format!(
                "Successfully imported {imported} material(s) into the catalog."
            )));
            scan_model();
        } else {
            error_message.set(Some("No regular missing materials could be imported.".to_string()));
        }
    };

    // Action: Git pull synchronization
    let run_git_sync = move |_| {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let config = APP_CONFIG.read();
            let catalog_dir = config.catalog_dir().cloned();
            let remote_url = config.catalog_remote_url().to_string();

            let Some(cat_path) = catalog_dir else {
                git_sync_result.set(Some(Err(
                    "Catalog directory is not configured in settings.".to_string(),
                )));
                return;
            };

            is_git_syncing.set(true);
            git_sync_result.set(None);

            spawn(async move {
                let cat_path_clone = cat_path.clone();
                let remote_url_clone = remote_url.clone();
                let mut registry = registry;

                let sync_task = std::thread::spawn(move || {
                    let sync = RegistrySync::new(&cat_path_clone, &remote_url_clone);
                    sync.init_or_clone().and_then(|()| sync.pull_updates())
                });

                let outcome = match sync_task.join() {
                    Ok(res) => match res {
                        Ok(()) => {
                            match registry.write().rebuild_index() {
                                Ok(count) => Ok(format!(
                                    "Git sync completed successfully. Catalog index refreshed ({count} assets indexed)."
                                )),
                                Err(e) => Ok(format!(
                                    "Git pull succeeded, but rebuilding index returned: {e}"
                                )),
                            }
                        }
                        Err(e) => Err(format!("Git synchronization failed: {e}")),
                    },
                    Err(_) => Err("Git background synchronization panicked.".to_string()),
                };

                is_git_syncing.set(false);
                git_sync_result.set(Some(outcome));
                scan_model();
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            git_sync_result.set(Some(Err(
                "Git catalog synchronization is not supported in web/wasm mode.".to_string(),
            )));
        }
    };

    // Filtered rows for the view
    let filtered_rows = use_memo(move || {
        let all = rows.read();
        let query = search_query.read();
        let cat_filter = *selected_category.read();
        let status_filter = *selected_status_filter.read();

        all.iter()
            .filter(|row| matches_filter(row, &query, cat_filter, status_filter))
            .cloned()
            .collect::<Vec<_>>()
    });

    // Count statistics
    let outdated_count = rows
        .read()
        .iter()
        .filter(|r| r.status == AssetSyncStatus::ModelOutdated)
        .count();
    let missing_regular_count = rows
        .read()
        .iter()
        .filter(|r| r.status == AssetSyncStatus::MissingInCatalogRegular)
        .count();
    let adhoc_count = rows
        .read()
        .iter()
        .filter(|r| r.status == AssetSyncStatus::MissingInCatalogAdHoc)
        .count();

    rsx! {
        AlertDialog {
            open: open(),
            on_open_change: move |v: bool| open.set(v),
            max_width: "68rem".to_string(),
            AlertDialogDescription {
                Card {
                    CardHeader {
                        CardTitle {
                            span { class: "d-flex align-items-center gap-2",
                                Icon { icon: FaArrowsRotate }
                                "Asset Catalog Synchronization"
                            }
                        }
                        CardAction {
                            Button {
                                title: "Re-scan loaded model and compare with catalog",
                                variant: ButtonVariant::Outline,
                                size: ButtonSize::Sm,
                                disabled: is_scanning(),
                                onclick: move |_| scan_model(),
                                Icon { icon: FaArrowsRotate }
                                if is_scanning() {
                                    "Scanning..."
                                } else {
                                    "Refresh"
                                }
                            }
                        }
                    }
                    CardContent {
                        // Git Registry Synchronization Banner
                        div { class: "p-3 mb-4 rounded border bg-light text-dark",
                            div { class: "d-flex justify-content-between align-items-center flex-wrap gap-3",
                                div {
                                    div { class: "fw-bold small d-flex align-items-center gap-1",
                                        Icon { icon: FaCodeBranch }
                                        "Shared Git Catalog Repository"
                                    }
                                    div { class: "text-muted small",
                                        "{APP_CONFIG.read().catalog_remote_url()}"
                                    }
                                    if let Some(dir) = APP_CONFIG.read().catalog_dir() {
                                        div { class: "text-muted small font-monospace",
                                            "{dir.display()}"
                                        }
                                    }
                                }
                                div {
                                    Button {
                                        variant: ButtonVariant::Primary,
                                        size: ButtonSize::Sm,
                                        disabled: is_git_syncing(),
                                        onclick: run_git_sync,
                                        Icon { icon: FaCloudArrowDown }
                                        if is_git_syncing() {
                                            "Syncing with Git..."
                                        } else {
                                            "Pull Updates from Git"
                                        }
                                    }
                                }
                            }

                            // Git sync result notification
                            if let Some(ref res) = *git_sync_result.read() {
                                match res {
                                    Ok(msg) => rsx! {
                                        div { class: "alert alert-success mt-2 mb-0 py-1 px-2 small d-flex align-items-center gap-2",
                                            Icon { icon: FaCheck }
                                            "{msg}"
                                        }
                                    },
                                    Err(err) => rsx! {
                                        div { class: "alert alert-danger mt-2 mb-0 py-1 px-2 small d-flex align-items-center gap-2",
                                            Icon { icon: FaCircleExclamation }
                                            "{err}"
                                        }
                                    },
                                }
                            }
                        }

                        // Status / Error alerts
                        if let Some(msg) = status_message.read().as_ref() {
                            div { class: "alert alert-info py-2 px-3 mb-3 small d-flex justify-content-between align-items-center",
                                span { "{msg}" }
                                button {
                                    r#type: "button",
                                    class: "btn-close btn-close-white small",
                                    onclick: move |_| status_message.set(None),
                                }
                            }
                        }
                        if let Some(err) = error_message.read().as_ref() {
                            div { class: "alert alert-danger py-2 px-3 mb-3 small d-flex justify-content-between align-items-center",
                                span { "{err}" }
                                button {
                                    r#type: "button",
                                    class: "btn-close btn-close-white small",
                                    onclick: move |_| error_message.set(None),
                                }
                            }
                        }

                        // Toolbar: Category Filter, Status Filter, Search, Batch Actions
                        div { class: "row mb-3 g-2 align-items-center",
                            // Category Selector
                            div { class: "col-auto",
                                div {
                                    class: "btn-group btn-group-sm",
                                    role: "group",
                                    button {
                                        r#type: "button",
                                        class: if selected_category().is_none() { "btn btn-primary" } else { "btn btn-outline-secondary" },
                                        onclick: move |_| selected_category.set(None),
                                        "All Assets ({rows.read().len()})"
                                    }
                                    button {
                                        r#type: "button",
                                        class: if selected_category() == Some(AssetCategory::Material) { "btn btn-primary" } else { "btn btn-outline-secondary" },
                                        onclick: move |_| selected_category.set(Some(AssetCategory::Material)),
                                        "Materials ({rows.read().len()})"
                                    }
                                    button {
                                        r#type: "button",
                                        class: "btn btn-outline-secondary disabled",
                                        title: "Coatings catalog synchronization coming soon",
                                        "Coatings (0)"
                                    }
                                }
                            }

                            // Status Filter Dropdown
                            div { class: "col-auto",
                                select {
                                    class: "form-select form-select-sm",
                                    value: match selected_status_filter() {
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
                                        selected_status_filter.set(val);
                                    },
                                    option { value: "all", "All Statuses" }
                                    option { value: "needs_action",
                                        "Needs Action ({outdated_count + missing_regular_count + adhoc_count})"
                                    }
                                    option { value: "outdated", "Outdated in Model ({outdated_count})" }
                                    option { value: "missing",
                                        "Missing in Catalog ({missing_regular_count + adhoc_count})"
                                    }
                                    option { value: "adhoc", "Ad-Hoc Drafts ({adhoc_count})" }
                                    option { value: "uptodate", "Up to date" }
                                }
                            }

                            // Search bar
                            div { class: "col",
                                input {
                                    class: "form-control form-control-sm",
                                    r#type: "text",
                                    placeholder: "Search by asset name or vendor...",
                                    value: "{search_query}",
                                    oninput: move |e| search_query.set(e.value()),
                                }
                            }

                            // Batch Action Buttons
                            div { class: "col-auto d-flex gap-2",
                                if outdated_count > 0 {
                                    Button {
                                        variant: ButtonVariant::Primary,
                                        size: ButtonSize::Sm,
                                        title: "Update all outdated materials to the latest catalog version",
                                        onclick: update_all_outdated,
                                        Icon { icon: FaCloudArrowDown }
                                        "Update All ({outdated_count})"
                                    }
                                }
                                if missing_regular_count > 0 {
                                    Button {
                                        variant: ButtonVariant::Success,
                                        size: ButtonSize::Sm,
                                        title: "Import all missing regular materials into the local catalog",
                                        onclick: import_all_missing,
                                        Icon { icon: FaCloudArrowUp }
                                        "Import All ({missing_regular_count})"
                                    }
                                }
                            }
                        }

                        // Assets Table View
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
                                        for row in filtered_rows.read().iter() {
                                            tr { key: "{row.id}",
                                                // Name
                                                td { class: "fw-bold", "{row.name}" }
                                                // Category
                                                td {
                                                    span { class: "badge bg-dark",
                                                        "{row.category.label()}"
                                                    }
                                                }
                                                // Manufacturer
                                                td { class: "text-muted",
                                                    "{row.manufacturer.as_deref().unwrap_or(\"-\")}"
                                                }
                                                // Model Version
                                                td {
                                                    if row.model_version == 0 {
                                                        span { class: "badge bg-warning text-dark",
                                                            "v0 (Ad-Hoc)"
                                                        }
                                                    } else {
                                                        span { class: "badge bg-secondary",
                                                            "v{row.model_version}"
                                                        }
                                                    }
                                                }
                                                // Catalog Version
                                                td {
                                                    if let Some(v) = row.catalog_version {
                                                        span { class: "badge bg-info text-dark",
                                                            "v{v}"
                                                        }
                                                    } else {
                                                        span { class: "text-muted", "-" }
                                                    }
                                                }
                                                // Used in Nodes
                                                td {
                                                    div {
                                                        class: "small text-truncate",
                                                        style: "max-width: 14rem;",
                                                        title: row.used_by_nodes
                                                            .iter()
                                                            .map(|u| u.node_name.as_str())
                                                            .collect::<Vec<_>>()
                                                            .join(", "),
                                                        "{row.used_by_nodes.iter().map(|u| u.node_name.as_str()).collect::<Vec<_>>().join(\", \")}"
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
                                                // Actions Column
                                                td { class: "text-end",
                                                    match row.status {
                                                        AssetSyncStatus::ModelOutdated => {
                                                            let row_clone = row.clone();
                                                            rsx! {
                                                                Button {
                                                                    title: "Update all node properties in the model to the newer catalog version",
                                                                    variant: ButtonVariant::Primary,
                                                                    size: ButtonSize::Sm,
                                                                    onclick: move |_| update_material_in_model(row_clone.clone()),
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
                                                                    onclick: move |_| import_regular_to_catalog.call(row_clone.clone()),
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
                                                                    onclick: move |_| {
                                                                        if let Some(mat) = &row_clone.material {
                                                                            editing_material.set(mat.clone());
                                                                            editing_usages.set(row_clone.used_by_nodes.clone());
                                                                            open_material_editor.set(true);
                                                                        }
                                                                    },
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
                                                                    onclick: move |_| import_regular_to_catalog.call(row_clone.clone()),
                                                                    Icon { icon: FaCloudArrowUp }
                                                                    "Update Catalog Item"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        if filtered_rows.read().is_empty() {
                                            tr {
                                                td {
                                                    colspan: "8",
                                                    class: "text-center text-muted py-4",
                                                    if rows.read().is_empty() {
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
            }
            AlertDialogActions {
                AlertDialogCancel { on_click: move |_| open.set(false), "Close" }
            }
        }

        // Material Editor Dialog for reviewing and publishing Ad-Hoc materials
        MaterialEditor {
            open: open_material_editor,
            material: editing_material,
            readonly: false,
            base_id: "catalogSyncMaterialEditor".to_string(),
            on_change: on_material_changed,
            on_save: on_material_save,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_sync_status() {
        // Up to date
        assert_eq!(
            determine_sync_status(1, Some(1)),
            AssetSyncStatus::UpToDate
        );
        assert_eq!(
            determine_sync_status(5, Some(5)),
            AssetSyncStatus::UpToDate
        );

        // Model outdated
        assert_eq!(
            determine_sync_status(1, Some(2)),
            AssetSyncStatus::ModelOutdated
        );
        assert_eq!(
            determine_sync_status(2, Some(5)),
            AssetSyncStatus::ModelOutdated
        );
        // Ad-hoc in model (v=0) but catalog already has an entry
        assert_eq!(
            determine_sync_status(0, Some(1)),
            AssetSyncStatus::ModelOutdated
        );

        // Model newer than catalog
        assert_eq!(
            determine_sync_status(3, Some(2)),
            AssetSyncStatus::ModelNewer
        );

        // Missing in catalog: regular (v >= 1)
        assert_eq!(
            determine_sync_status(1, None),
            AssetSyncStatus::MissingInCatalogRegular
        );
        assert_eq!(
            determine_sync_status(4, None),
            AssetSyncStatus::MissingInCatalogRegular
        );

        // Missing in catalog: ad-hoc draft (v == 0)
        assert_eq!(
            determine_sync_status(0, None),
            AssetSyncStatus::MissingInCatalogAdHoc
        );
    }

    fn sample_row(
        name: &str,
        category: AssetCategory,
        mfr: Option<&str>,
        status: AssetSyncStatus,
    ) -> AssetSyncRow {
        AssetSyncRow {
            id: Uuid::new_v4(),
            name: name.to_string(),
            category,
            manufacturer: mfr.map(str::to_string),
            model_version: 1,
            catalog_version: Some(1),
            status,
            used_by_nodes: Vec::new(),
            material: None,
        }
    }

    #[test]
    fn test_matches_filter_status() {
        let row_uptodate = sample_row("N-BK7", AssetCategory::Material, Some("Schott"), AssetSyncStatus::UpToDate);
        let row_outdated = sample_row("Fused Silica", AssetCategory::Material, Some("Corning"), AssetSyncStatus::ModelOutdated);
        let row_missing_reg = sample_row("Custom Glass", AssetCategory::Material, None, AssetSyncStatus::MissingInCatalogRegular);
        let row_adhoc = sample_row("Draft Resin", AssetCategory::Material, None, AssetSyncStatus::MissingInCatalogAdHoc);

        // Filter: All
        assert!(matches_filter(&row_uptodate, "", None, StatusFilter::All));
        assert!(matches_filter(&row_outdated, "", None, StatusFilter::All));
        assert!(matches_filter(&row_missing_reg, "", None, StatusFilter::All));
        assert!(matches_filter(&row_adhoc, "", None, StatusFilter::All));

        // Filter: NeedsAction
        assert!(!matches_filter(&row_uptodate, "", None, StatusFilter::NeedsAction));
        assert!(matches_filter(&row_outdated, "", None, StatusFilter::NeedsAction));
        assert!(matches_filter(&row_missing_reg, "", None, StatusFilter::NeedsAction));
        assert!(matches_filter(&row_adhoc, "", None, StatusFilter::NeedsAction));

        // Filter: Outdated
        assert!(!matches_filter(&row_uptodate, "", None, StatusFilter::Outdated));
        assert!(matches_filter(&row_outdated, "", None, StatusFilter::Outdated));
        assert!(!matches_filter(&row_missing_reg, "", None, StatusFilter::Outdated));

        // Filter: Missing
        assert!(!matches_filter(&row_uptodate, "", None, StatusFilter::Missing));
        assert!(!matches_filter(&row_outdated, "", None, StatusFilter::Missing));
        assert!(matches_filter(&row_missing_reg, "", None, StatusFilter::Missing));
        assert!(matches_filter(&row_adhoc, "", None, StatusFilter::Missing));

        // Filter: AdHocOnly
        assert!(!matches_filter(&row_uptodate, "", None, StatusFilter::AdHocOnly));
        assert!(!matches_filter(&row_missing_reg, "", None, StatusFilter::AdHocOnly));
        assert!(matches_filter(&row_adhoc, "", None, StatusFilter::AdHocOnly));

        // Filter: UpToDate
        assert!(matches_filter(&row_uptodate, "", None, StatusFilter::UpToDate));
        assert!(!matches_filter(&row_outdated, "", None, StatusFilter::UpToDate));
    }

    #[test]
    fn test_matches_filter_category_and_search() {
        let row = sample_row("N-BK7", AssetCategory::Material, Some("Schott"), AssetSyncStatus::UpToDate);

        // Category filter
        assert!(matches_filter(&row, "", Some(AssetCategory::Material), StatusFilter::All));
        assert!(!matches_filter(&row, "", Some(AssetCategory::Coating), StatusFilter::All));

        // Text search matching name
        assert!(matches_filter(&row, "bk7", None, StatusFilter::All));
        assert!(matches_filter(&row, "N-BK7", None, StatusFilter::All));

        // Text search matching manufacturer
        assert!(matches_filter(&row, "schott", None, StatusFilter::All));

        // Text search non-matching
        assert!(!matches_filter(&row, "corning", None, StatusFilter::All));
    }
}

