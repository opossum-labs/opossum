pub mod git_banner;
pub mod service;
pub mod table;
pub mod toolbar;
pub mod types;

pub use git_banner::GitSyncBanner;
pub use table::CatalogTable;
pub use toolbar::CatalogToolbar;
pub use types::*;

use crate::{
    APP_CONFIG,
    components::{
        asset_editor::material_editor::{MaterialChangeEvent, MaterialEditor},
        primitives::{
            alert_dialog::{
                AlertDialog, AlertDialogActions, AlertDialogCancel, AlertDialogDescription,
            },
            button::{Button, ButtonSize, ButtonVariant},
            card::{Card, CardAction, CardContent, CardHeader, CardTitle},
        },
    },
};
use dioxus::prelude::*;
use dioxus_free_icons::{Icon, icons::fa_solid_icons::FaArrowsRotate};
use opossum_core::{material::Material, refractive_index::RefrIndexSellmeier1};
use opossum_registry::AssetRegistry;

/// Main modal dialog component for asset catalog synchronization.
#[component]
pub fn CatalogSynchronizer(
    /// Controls whether the synchronizer modal dialog is visible.
    open: Signal<bool>,
) -> Element {
    let registry = use_context::<Signal<AssetRegistry<Material>>>();

    // Local state
    let mut rows = use_signal(Vec::<AssetSyncRow>::new);
    let mut is_scanning = use_signal(|| false);
    let mut error_message = use_signal(|| Option::<String>::None);
    let mut status_message = use_signal(|| Option::<String>::None);

    // Git sync state
    let mut is_git_syncing = use_signal(|| false);
    let mut git_sync_result = use_signal(|| Option::<Result<String, String>>::None);

    // Filter and search state
    let search_query = use_signal(String::new);
    let selected_category = use_signal(|| Option::<AssetCategory>::None);
    let selected_status_filter = use_signal(|| StatusFilter::All);

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

    // Helper: Scans the model using the service layer
    let scan_model = move || {
        spawn(async move {
            is_scanning.set(true);
            error_message.set(None);

            // Clone registry value so no GenerationalRef is held across the await point
            let reg = registry.cloned();
            match service::fetch_and_scan_model(&reg).await {
                Ok(computed_rows) => rows.set(computed_rows),
                Err(err) => error_message.set(Some(err)),
            }

            is_scanning.set(false);
        });
    };

    // Auto-scan whenever the dialog opens
    use_effect(move || {
        if open() {
            scan_model();
        }
    });

    // Action: Update a single outdated material to the catalog version
    let update_material_in_model = move |row: AssetSyncRow| {
        spawn(async move {
            let latest_mat = match registry.read().load(row.id, None) {
                Ok(m) => m,
                Err(e) => {
                    error_message.set(Some(format!("Failed to load material from catalog: {e}")));
                    return;
                }
            };

            match service::update_material_in_nodes(&latest_mat, &row.used_by_nodes).await {
                Ok(()) => {
                    status_message.set(Some(format!(
                        "Updated '{}' to v{} in {} node(s).",
                        row.name,
                        latest_mat.version(),
                        row.used_by_nodes.len()
                    )));
                    *crate::NODE_DETAILS_REFRESH.write() += 1;
                    scan_model();
                }
                Err(errors) => {
                    error_message.set(Some(format!(
                        "Errors updating nodes: {}",
                        errors.join("; ")
                    )));
                }
            }
        });
    };

    // Action: Import a regular material (v >= 1) into the catalog
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
                    error_message.set(Some(format!("Failed to publish to catalog: {e}")));
                }
            }
        }
    });

    // Callback: Handle property edits from MaterialEditor
    let on_material_changed = use_callback(move |e: MaterialChangeEvent| {
        e.action.apply(&mut editing_material.write());
    });

    // Callback: Publish edited Ad-Hoc material and update nodes
    let on_material_save = use_callback(move |()| {
        let mut registry = registry;
        let mut mat = editing_material.read().clone();
        match registry.write().publish(&mut mat) {
            Ok(path) => {
                let published_version = mat.version();
                let published_name = mat.name().to_string();
                let usages = editing_usages.read().clone();

                spawn(async move {
                    let _ = service::update_material_in_nodes(&mat, &usages).await;
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

    // Action: Batch update all outdated materials
    let update_all_outdated = move |()| {
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
                // Load material synchronously first so the GenerationalRef is dropped before the await point
                let load_result = registry.read().load(row.id, None);
                if let Ok(latest_mat) = load_result
                    && service::update_material_in_nodes(&latest_mat, &row.used_by_nodes)
                        .await
                        .is_ok()
                {
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

    // Action: Batch import all regular missing materials
    let import_all_missing = move |()| {
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
            error_message.set(Some(
                "No regular missing materials could be imported.".to_string(),
            ));
        }
    };

    // Action: Trigger Git sync
    let run_git_sync = move |()| {
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
                let mut reg = registry;
                let outcome = service::sync_git_catalog(&cat_path, &remote_url, &mut reg.write());
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

    // Computed filtered rows
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

    // Summary statistics
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
                        // Git Banner subcomponent
                        GitSyncBanner {
                            is_syncing: is_git_syncing(),
                            sync_result: git_sync_result.read().clone(),
                            on_sync: run_git_sync,
                        }

                        // Status and error banners
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

                        // Toolbar subcomponent
                        CatalogToolbar {
                            selected_category,
                            selected_status_filter,
                            search_query,
                            total_count: rows.read().len(),
                            outdated_count,
                            missing_regular_count,
                            adhoc_count,
                            on_update_all: update_all_outdated,
                            on_import_all: import_all_missing,
                        }

                        // Table subcomponent
                        CatalogTable {
                            rows: filtered_rows.read().clone(),
                            total_model_assets: rows.read().len(),
                            on_update_in_model: update_material_in_model,
                            on_import_to_catalog: move |row| import_regular_to_catalog.call(row),
                            on_edit_adhoc: move |row: AssetSyncRow| {
                                if let Some(mat) = &row.material {
                                    editing_material.set(mat.clone());
                                    editing_usages.set(row.used_by_nodes.clone());
                                    open_material_editor.set(true);
                                }
                            },
                        }
                    }
                }
            }
            AlertDialogActions {
                AlertDialogCancel { on_click: move |_| open.set(false), "Close" }
            }
        }

        // Material Editor dialog for ad-hoc drafts
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
