use crate::api::{get_document, update_node_property};
use opossum_core::{
    material::Material,
    opm_document::OpmDocument,
    properties::{Proptype, proptype::AssetRef},
};
#[cfg(not(target_arch = "wasm32"))]
use opossum_registry::RegistrySync;
use opossum_registry::{AssetRegistry, CatalogSource, asset::RegisterableAsset};
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

use super::types::{AssetCategory, AssetSyncRow, NodeUsageInfo, determine_dual_catalog_status};

/// Extracts all inline materials and their node usage occurrences from an optical document.
#[must_use]
pub fn extract_materials_from_document(
    document: &OpmDocument,
) -> HashMap<Uuid, (Material, Vec<NodeUsageInfo>)> {
    let mut materials_map: HashMap<Uuid, (Material, Vec<NodeUsageInfo>)> = HashMap::new();

    let all_nodes = document.scenery().collect_all_nodes_recursive();

    for node_ref in all_nodes {
        let node_id = node_ref.node_attr().uuid();
        let node_name = node_ref.node_attr().name().to_string();

        for (prop_name, prop) in node_ref.node_attr().properties() {
            if let Proptype::Material(AssetRef::Inline(mat)) = prop.prop() {
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
    materials_map
}

/// Compares extracted model materials against both the `official` and `user` catalog indices.
pub fn build_sync_rows(
    materials_map: HashMap<Uuid, (Material, Vec<NodeUsageInfo>)>,
    registry: &AssetRegistry<Material>,
) -> Vec<AssetSyncRow> {
    let mut rows = Vec::with_capacity(materials_map.len());
    let official_idx = registry.official_index();
    let user_idx = registry.user_index();

    for (mat_id, (mat, usages)) in materials_map {
        let is_official = registry.is_official(mat_id);
        let official_version = official_idx.get(&mat_id).map(|e| e.common.latest_version);
        let in_user_catalog = registry.in_user_catalog(mat_id);
        let user_version = user_idx.get(&mat_id).map(|e| e.common.latest_version);

        let (origin, catalog_version, status) = determine_dual_catalog_status(
            mat.version(),
            is_official,
            official_version,
            in_user_catalog,
            user_version,
        );

        rows.push(AssetSyncRow {
            id: mat_id,
            name: mat.name().to_string(),
            category: AssetCategory::Material,
            manufacturer: mat.manufacturer().map(str::to_string),
            origin,
            model_version: mat.version(),
            catalog_version,
            status,
            used_by_nodes: usages,
            material: Some(mat),
        });
    }

    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

/// Fetches the active document from the backend API, parses it, and compares all assets with the dual catalog.
pub async fn fetch_and_scan_model(
    registry: &AssetRegistry<Material>,
) -> Result<Vec<AssetSyncRow>, String> {
    let opm_str = get_document()
        .await
        .map_err(|e| format!("Failed to fetch document: {e}"))?;

    let document =
        OpmDocument::from_string(&opm_str).map_err(|e| format!("Failed to parse document: {e}"))?;

    let materials_map = extract_materials_from_document(&document);
    Ok(build_sync_rows(materials_map, registry))
}

/// Updates the material property on all referencing model nodes via API calls.
pub async fn update_material_in_nodes(
    material: &Material,
    usages: &[NodeUsageInfo],
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    for usage in usages {
        let prop = Proptype::Material(AssetRef::Inline(material.clone()));
        if let Err(e) =
            update_node_property(usage.node_id, (usage.property_key.clone(), prop)).await
        {
            errors.push(format!("{}: {e}", usage.node_name));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Resolves a user catalog conflict by overwriting with a new version in the user catalog,
/// saving as a renamed copy with a fresh UUID in the user catalog, or canceling.
pub fn resolve_user_catalog_conflict_overwrite(
    material: &mut Material,
    registry: &mut AssetRegistry<Material>,
) -> Result<std::path::PathBuf, String> {
    material.header_mut().version = 0;
    registry
        .publish_to(CatalogSource::User, material)
        .map_err(|e| format!("Failed to overwrite material in user catalog: {e}"))
}

/// Saves a conflicting model material as a new entry (new UUID, new name) in the user catalog.
pub fn resolve_user_catalog_conflict_rename(
    material: &Material,
    new_name: String,
    registry: &mut AssetRegistry<Material>,
) -> Result<Material, String> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err("New material name cannot be empty.".to_string());
    }
    let mut cloned = material.clone();
    cloned.header_mut().id = Uuid::new_v4();
    cloned.header_mut().version = 0;
    cloned.header_mut().name = trimmed.to_string();
    registry
        .publish_to(CatalogSource::User, &mut cloned)
        .map_err(|e| format!("Failed to save renamed copy in user catalog: {e}"))?;
    Ok(cloned)
}

/// Executes a Git pull on the official catalog directory on a background thread and rebuilds indices.
#[cfg(not(target_arch = "wasm32"))]
pub fn sync_git_catalog(
    catalog_dir: &Path,
    remote_url: &str,
    registry: &mut AssetRegistry<Material>,
) -> Result<String, String> {
    let official_dir = catalog_dir.join("official");
    let remote_url_str = remote_url.to_string();

    let sync_task = std::thread::spawn(move || {
        let sync = RegistrySync::new_official(
            &official_dir,
            if remote_url_str.trim().is_empty() {
                None
            } else {
                Some(remote_url_str.as_str())
            },
        );
        sync.ensure_repository_initialized()
            .and_then(|()| sync.pull_updates())
    });

    match sync_task.join() {
        Ok(Ok(())) => match registry.rebuild_index() {
            Ok(count) => Ok(format!(
                "Official catalog Git sync completed. Indices refreshed ({count} total assets indexed)."
            )),
            Err(e) => Ok(format!(
                "Git pull succeeded, but rebuilding index returned: {e}"
            )),
        },
        Ok(Err(e)) => Err(format!("Git synchronization failed: {e}")),
        Err(_) => Err("Git background synchronization panicked.".to_string()),
    }
}
