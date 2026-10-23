use opossum_core::material::Material;
use uuid::Uuid;

/// Represents an asset category for catalog synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetCategory {
    Material,
    #[expect(dead_code)]
    Coating,
}

impl AssetCategory {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Material => "Material",
            Self::Coating => "Coating",
        }
    }
}

/// Comparison status between an asset in the model and the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetSyncStatus {
    UpToDate,
    ModelOutdated,
    MissingInCatalogRegular,
    MissingInCatalogAdHoc,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusFilter {
    #[default]
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
