use super::CatalogSynchronizer;
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
    icons::fa_solid_icons::{FaArrowsRotate, FaCheck, FaPencil, FaPlus, FaTrash},
};
use dioxus_primitives::alert_dialog::AlertDialogContent;
use opossum_core::{material::Material, refractive_index::RefrIndexSellmeier1};
use opossum_registry::{
    AssetRegistry, CatalogSource,
    index::{IndexEntry, MaterialIndexData},
};
use uuid::Uuid;

/// Metadata of an asset queued for deletion confirmation (user catalog only).
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
    let mut registry = use_context::<Signal<AssetRegistry<Material>>>();

    let is_select_mode = on_select.is_some();

    let mut open_materialeditor = use_signal(|| false);
    let mut material_state = use_signal(Material::default);
    let mut show_delete_dialog = use_signal(|| false);
    let mut pending_delete = use_signal(|| Option::<DeleteTarget>::None);
    let mut show_synchronizer = use_signal(|| false);

    let mut search_text = use_signal(String::new);
    let mut min_nd = use_signal(|| Option::<f64>::None);
    let mut max_nd = use_signal(|| Option::<f64>::None);

    let min_nd_str = min_nd().map_or_default(|v| v.to_string());
    let max_nd_str = max_nd().map_or_default(|v| v.to_string());

    let filtered_entries = use_memo(move || {
        let text_query = search_text.read();
        let reg = registry.read();
        let trimmed_query = text_query.trim();

        let mut results: Vec<IndexEntry<MaterialIndexData>> = if trimmed_query.is_empty() {
            reg.all_entries().into_iter().cloned().collect()
        } else {
            reg.search_text(trimmed_query)
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

        results.sort_by_cached_key(|entry| entry.common.name.to_lowercase());
        results
    });

    let on_material_changed = use_callback(move |e: MaterialChangeEvent| {
        e.action.apply(&mut material_state.write());
    });

    let on_material_save = use_callback(move |()| {
        let mut mat_guard = material_state.write();
        match registry
            .write()
            .publish_to(CatalogSource::User, &mut *mat_guard)
        {
            Ok(_) => {
                on_action.call(MaterialCatalogEvent::MaterialAdded);
            }
            Err(e) => {
                log::error!("Failed to publish material to user catalog: {e}");
            }
        }
    });

    let on_create_new = use_callback(move |_| {
        material_state.set(Material::new_draft(
            "New Material",
            None,
            None,
            RefrIndexSellmeier1::default().into(),
        ));
        open_materialeditor.set(true);
    });

    // Delegates draft creation to AssetRegistry:
    // - Official material -> new UUID + version 0 (fork to user catalog)
    // - User material -> same UUID + version 0 (next version in user catalog)
    let on_edit_material =
        use_callback(
            move |id: Uuid| match registry.read().prepare_user_draft(id, None) {
                Ok(draft) => {
                    material_state.set(draft);
                    open_materialeditor.set(true);
                }
                Err(err) => {
                    log::error!("Failed to prepare user draft from registry: {err}");
                }
            },
        );

    let handle_execute_delete = use_callback(move |target: DeleteTarget| {
        match registry
            .write()
            .delete_latest_version_from(CatalogSource::User, target.id)
        {
            Ok(_) => {
                on_action.call(MaterialCatalogEvent::MaterialDeleted);
            }
            Err(e) => {
                log::error!("Failed to delete user material version: {e}");
            }
        }
    });

    let handle_confirm_delete = use_callback(move |_| {
        if let Some(target) = pending_delete.read().clone() {
            handle_execute_delete.call(target);
        }
        show_delete_dialog.set(false);
    });

    let handle_cancel_delete = use_callback(move |_| {
        show_delete_dialog.set(false);
    });

    let snapshot_rows: Vec<(IndexEntry<MaterialIndexData>, bool, bool)> = {
        let reg = registry.read();
        filtered_entries
            .read()
            .iter()
            .cloned()
            .map(|entry| {
                let is_off = reg.is_official(entry.common.id);
                let is_usr = reg.in_user_catalog(entry.common.id);
                (entry, is_off, is_usr)
            })
            .collect()
    };
    let has_rows = !snapshot_rows.is_empty();

    let delete_prompt_text = pending_delete.read().as_ref().map_or_else(
        String::new,
        |target| {
            let prev_v = target.latest_version.saturating_sub(1);
            if target.total_versions_count <= 1 {
                format!(
                    "Warning: This is the only version of '{}' in the user catalog. Deleting version v{} will permanently remove this user material.",
                    target.name, target.latest_version
                )
            } else {
                format!(
                    "Are you sure you want to delete the latest version (v{}) of '{}' from the user catalog? The material will revert to v{prev_v}.",
                    target.name, target.latest_version
                )
            }
        },
    );

    rsx! {
        AlertDialog {
            open: open(),
            on_open_change: move |v: bool| open.set(v),
            max_width: "64rem".to_string(),
            AlertDialogDescription {
                div {
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
                                    div { class: "d-flex gap-2",
                                        Button {
                                            title: "Compare and synchronize model assets with the catalog",
                                            variant: ButtonVariant::Outline,
                                            onclick: move |_| show_synchronizer.set(true),
                                            Icon { icon: FaArrowsRotate }
                                            "Sync with Model"
                                        }
                                        Button {
                                            variant: ButtonVariant::Success,
                                            onclick: on_create_new,
                                            Icon { icon: FaPlus }
                                            "New Material"
                                        }
                                    }
                                }
                            }
                        }
                        CardContent {
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
                                        value: "{min_nd_str}",
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
                                        value: "{max_nd_str}",
                                        oninput: move |e| max_nd.set(e.value().parse().ok()),
                                    }
                                }
                            }

                            ScrollArea { height: "25rem",
                                div { class: "table-responsive",
                                    table { class: "table table-hover table-sm align-middle mb-0",
                                        thead { class: "table-light sticky-top",
                                            tr {
                                                th { "Name" }
                                                th { "Catalog" }
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
                                            if has_rows {
                                                for (entry , is_off , is_usr) in &snapshot_rows {
                                                    {
                                                        let row_key = if *is_usr {
                                                            format!("user-{}", entry.common.id)
                                                        } else {
                                                            format!("official-{}", entry.common.id)
                                                        };
                                                        let preferred_source = if *is_off {
                                                            CatalogSource::Official
                                                        } else {
                                                            CatalogSource::User
                                                        };
                                                        let edit_title = if *is_off {
                                                            "Fork official material into user catalog"
                                                        } else {
                                                            "Create new version in user catalog"
                                                        };
                                                        let desc_title = entry
                                                            .common
                                                            .description
                                                            .as_deref()
                                                            .unwrap_or_default()
                                                            .to_string();
                                                        let desc_short = truncate_description(entry.common.description.as_deref(), 35);
                                                        let mfg_display = entry
                                                            .common
                                                            .manufacturer
                                                            .as_deref()
                                                            .unwrap_or("-")
                                                            .to_string();
                                                        let nd_display = entry
                                                            .specific
                                                            .nd
                                                            .map_or_else(|| "-".to_string(), |nd| format!("{nd:.4}"));
                                                        let ver_display = format!("v{}", entry.common.latest_version);

                                                        rsx! {
                                                            tr { key: "{row_key}",
                                                                td { class: "fw-bold", "{entry.common.name}" }
                                                                td {
                                                                    div { class: "d-inline-flex gap-1 align-items-center",
                                                                        if *is_off {
                                                                            span { class: "badge bg-primary", "Official" }
                                                                        }
                                                                        if *is_usr {
                                                                            span { class: "badge bg-info text-dark", "User" }
                                                                        }
                                                                        if !*is_off && !*is_usr {
                                                                            span { class: "badge bg-secondary", "-" }
                                                                        }
                                                                    }
                                                                }
                                                                td { "{mfg_display}" }
                                                                td { title: "{desc_title}", class: "text-muted small", "{desc_short}" }
                                                                td { "{nd_display}" }
                                                                td {
                                                                    span { class: "badge bg-secondary", "{ver_display}" }
                                                                }
                                                                td { class: "text-end",
                                                                    div { class: "d-inline-flex justify-content-end gap-1",
                                                                        if let Some(select_handler) = on_select {
                                                                            Button {
                                                                                title: "Select this material for the current node",
                                                                                variant: ButtonVariant::Primary,
                                                                                onclick: {
                                                                                    let id = entry.common.id;
                                                                                    let version = entry.common.latest_version;
                                                                                    move |_| {
                                                                                        if let Ok(loaded) = registry
                                                                                            .read()
                                                                                            .load_from(preferred_source, id, Some(version))
                                                                                        {
                                                                                            select_handler.call(loaded);
                                                                                            open.set(false);
                                                                                        }
                                                                                    }
                                                                                },
                                                                                Icon { icon: FaCheck }
                                                                                "Select"
                                                                            }
                                                                        } else {
                                                                            Button {
                                                                                title: "{edit_title}",
                                                                                onclick: {
                                                                                    let id = entry.common.id;
                                                                                    move |_| on_edit_material.call(id)
                                                                                },
                                                                                Icon { icon: FaPencil }
                                                                            }
                                                                            if *is_usr {
                                                                                Button {
                                                                                    title: "Delete latest version from user catalog",
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
                                                            }
                                                        }
                                                    }
                                                }
                                            } else {
                                                tr {
                                                    td {
                                                        colspan: "7",
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
            }
            AlertDialogActions {
                AlertDialogAction { "Close" }
            }
        }

        if !is_select_mode {
            MaterialEditor {
                open: open_materialeditor,
                material: material_state,
                readonly: false,
                on_change: on_material_changed,
                on_save: on_material_save,
            }
        }

        AlertDialog {
            open: show_delete_dialog(),
            on_open_change: move |open_val: bool| {
                show_delete_dialog.set(open_val);
            },
            AlertDialogContent {
                AlertDialogTitle { "Delete User Material Version" }
                AlertDialogDescription { "{delete_prompt_text}" }
                AlertDialogActions {
                    AlertDialogCancel { on_click: handle_cancel_delete, "Cancel" }
                    AlertDialogAction { on_click: handle_confirm_delete, "Delete Version" }
                }
            }
        }
        CatalogSynchronizer { open: show_synchronizer }
    }
}
