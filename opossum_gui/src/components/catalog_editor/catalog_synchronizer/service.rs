use crate::api::{get_document, update_node_property};
use opossum_core::{
    material::Material,
    opm_document::OpmDocument,
    properties::{Proptype, proptype::AssetRef},
};
#[cfg(not(target_arch = "wasm32"))]
use opossum_registry::RegistrySync;
use opossum_registry::{AssetRegistry, asset::RegisterableAsset};
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

use super::types::{AssetCategory, AssetSyncRow, NodeUsageInfo, determine_sync_status};

/// Extracts all inline materials and their node usage occurrences from an optical document.
pub fn extract_materials_from_document(
    document: &OpmDocument,
) -> Result<HashMap<Uuid, (Material, Vec<NodeUsageInfo>)>, String> {
    let mut materials_map: HashMap<Uuid, (Material, Vec<NodeUsageInfo>)> = HashMap::new();

    let all_nodes = document
        .scenery()
        .collect_all_nodes_recursive()
        .map_err(|e| format!("Failed to collect scenery nodes: {e}"))?;

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

    Ok(materials_map)
}

/// Compares a map of extracted model materials against the catalog index and generates sorted sync rows.
pub fn build_sync_rows(
    materials_map: HashMap<Uuid, (Material, Vec<NodeUsageInfo>)>,
    registry: &AssetRegistry<Material>,
) -> Vec<AssetSyncRow> {
    let mut rows = Vec::with_capacity(materials_map.len());
    let reg_index = registry.index();

    for (mat_id, (mat, usages)) in materials_map {
        let cat_entry = reg_index.get(&mat_id);
        let cat_version = cat_entry.map(|e| e.common.latest_version);
        let status = determine_sync_status(mat.version(), cat_version);

        rows.push(AssetSyncRow {
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

    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

/// Fetches the active document from the backend API, parses it, and compares all assets with the catalog.
pub async fn fetch_and_scan_model(
    registry: &AssetRegistry<Material>,
) -> Result<Vec<AssetSyncRow>, String> {
    let opm_str = get_document()
        .await
        .map_err(|e| format!("Failed to fetch document: {e}"))?;

    let document =
        OpmDocument::from_string(&opm_str).map_err(|e| format!("Failed to parse document: {e}"))?;

    let materials_map = extract_materials_from_document(&document)?;
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

/// Executes a Git pull on a background thread and rebuilds the catalog index.
#[cfg(not(target_arch = "wasm32"))]
pub async fn sync_git_catalog(
    catalog_dir: &Path,
    remote_url: &str,
    registry: &mut AssetRegistry<Material>,
) -> Result<String, String> {
    let cat_path_buf = catalog_dir.to_path_buf();
    let remote_url_str = remote_url.to_string();

    let sync_task = std::thread::spawn(move || {
        let sync = RegistrySync::new(&cat_path_buf, &remote_url_str);
        sync.init_or_clone().and_then(|()| sync.pull_updates())
    });

    match sync_task.join() {
        Ok(Ok(())) => match registry.rebuild_index() {
            Ok(count) => Ok(format!(
                "Git sync completed successfully. Catalog index refreshed ({count} assets indexed)."
            )),
            Err(e) => Ok(format!(
                "Git pull succeeded, but rebuilding index returned: {e}"
            )),
        },
        Ok(Err(e)) => Err(format!("Git synchronization failed: {e}")),
        Err(_) => Err("Git background synchronization panicked.".to_string()),
    }
}
