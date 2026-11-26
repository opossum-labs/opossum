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

/// Indicates which catalog source an asset belongs to or targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogOrigin {
    /// Asset is registered in the shared official catalog.
    Official,
    /// Asset is registered in the local user catalog.
    User,
    /// Asset is not yet present in either catalog.
    None,
}

/// Comparison status between an asset in the model and the dual-catalog registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetSyncStatus {
    /// Model asset matches the catalog version (`official` or `user`).
    UpToDate,
    /// Catalog (`official` or `user`) has a newer version than the model.
    ModelOutdated,
    /// Model holds a newer version of an official asset; pull the official catalog first.
    OfficialSyncRecommended,
    /// Asset is not in official or user catalog and has version >= 1 (can be imported to user catalog).
    MissingInCatalogRegular,
    /// Asset is not in official or user catalog and has version == 0 (ad-hoc draft).
    MissingInCatalogAdHoc,
    /// Asset exists in the user catalog with a conflicting version or state; user must resolve.
    UserConflict,
}

/// Identifies a node and property that uses an asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeUsageInfo {
    pub node_id: Uuid,
    pub node_name: String,
    pub property_key: String,
}

/// A row representing an asset found in the model compared to the official and user catalogs.
#[derive(Debug, Clone, PartialEq)]
pub struct AssetSyncRow {
    pub id: Uuid,
    pub name: String,
    pub category: AssetCategory,
    pub manufacturer: Option<String>,
    pub origin: CatalogOrigin,
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
    ConflictsOnly,
    UpToDate,
}

/// Determines the synchronization status using the official catalog (by UUID) as single point of truth,
/// falling back to the user catalog and then to uncataloged model assets.
#[must_use]
pub const fn determine_dual_catalog_status(
    model_version: u32,
    is_official: bool,
    official_version: Option<u32>,
    in_user_catalog: bool,
    user_version: Option<u32>,
) -> (CatalogOrigin, Option<u32>, AssetSyncStatus) {
    if is_official {
        let cat_v = official_version;
        match cat_v {
            Some(latest_off_v) => {
                if latest_off_v > model_version {
                    (
                        CatalogOrigin::Official,
                        cat_v,
                        AssetSyncStatus::ModelOutdated,
                    )
                } else if latest_off_v == model_version && model_version >= 1 {
                    (CatalogOrigin::Official, cat_v, AssetSyncStatus::UpToDate)
                } else if model_version > latest_off_v {
                    (
                        CatalogOrigin::Official,
                        cat_v,
                        AssetSyncStatus::OfficialSyncRecommended,
                    )
                } else {
                    (
                        CatalogOrigin::Official,
                        cat_v,
                        AssetSyncStatus::ModelOutdated,
                    )
                }
            }
            None => (
                CatalogOrigin::Official,
                None,
                AssetSyncStatus::OfficialSyncRecommended,
            ),
        }
    } else if in_user_catalog {
        let cat_v = user_version;
        match cat_v {
            Some(latest_usr_v) => {
                if latest_usr_v > model_version {
                    (CatalogOrigin::User, cat_v, AssetSyncStatus::ModelOutdated)
                } else if latest_usr_v == model_version && model_version >= 1 {
                    (CatalogOrigin::User, cat_v, AssetSyncStatus::UpToDate)
                } else {
                    // Model version differs higher or v0 conflicts with existing user catalog entry
                    (CatalogOrigin::User, cat_v, AssetSyncStatus::UserConflict)
                }
            }
            None => (CatalogOrigin::User, None, AssetSyncStatus::UserConflict),
        }
    } else if model_version == 0 {
        (
            CatalogOrigin::None,
            None,
            AssetSyncStatus::MissingInCatalogAdHoc,
        )
    } else {
        (
            CatalogOrigin::None,
            None,
            AssetSyncStatus::MissingInCatalogRegular,
        )
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
        StatusFilter::ConflictsOnly => {
            if row.status != AssetSyncStatus::UserConflict {
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
