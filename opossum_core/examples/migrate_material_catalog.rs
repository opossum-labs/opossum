use opossum_core::{
    error::{OpmResult, OpossumError},
    material::{AssetHeader, Material, MechanicalProperties, OpticalProperties, ThermalProperties},
};
use ron::{extensions::Extensions, ser::PrettyConfig};
use serde::Deserialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// Legacy material layout where `AssetHeader` fields were flattened into a RON map.
#[derive(Debug, Deserialize)]
struct LegacyMaterial {
    #[serde(flatten)]
    header: AssetHeader,
    optical: OpticalProperties,
    #[serde(default)]
    thermal: Option<ThermalProperties>,
    #[serde(default)]
    mechanical: Option<MechanicalProperties>,
}

impl From<LegacyMaterial> for Material {
    fn from(legacy: LegacyMaterial) -> Self {
        Self {
            header: legacy.header,
            optical: legacy.optical,
            thermal: legacy.thermal,
            mechanical: legacy.mechanical,
        }
    }
}

fn collect_ron_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_ron_files(&path, out)?;
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ron"))
        {
            out.push(path);
        }
    }
    Ok(())
}

fn main() -> OpmResult<()> {
    let target_dir = env::args().nth(1).map_or_else(
        || {
            eprintln!("Usage: cargo run -p opossum_core --example migrate_material_catalog -- <catalog_dir>");
            std::process::exit(1);
        },
        PathBuf::from,
    );

    if !target_dir.is_dir() {
        return Err(OpossumError::Other(format!(
            "Directory does not exist: {}",
            target_dir.display()
        )));
    }

    let mut files = Vec::new();
    collect_ron_files(&target_dir, &mut files)
        .map_err(|e| OpossumError::Other(format!("Failed to scan directory: {e}")))?;
    files.sort();

    let pretty_cfg = PrettyConfig::new()
        .extensions(Extensions::UNWRAP_VARIANT_NEWTYPES | Extensions::IMPLICIT_SOME)
        .new_line("\n");

    let mut migrated_count = 0usize;
    let mut skipped_count = 0usize;

    for path in &files {
        let raw = fs::read_to_string(path)
            .map_err(|e| OpossumError::Other(format!("Failed to read {}: {e}", path.display())))?;

        // Skip files that are already in the new v1 Material format
        if ron::from_str::<Material>(&raw).is_ok() {
            skipped_count += 1;
            continue;
        }

        let legacy: LegacyMaterial = ron::from_str(&raw)
            .map_err(|e| OpossumError::Other(format!("Failed to parse {}: {e}", path.display())))?;

        let modern = Material::from(legacy);
        let serialized = ron::ser::to_string_pretty(&modern, pretty_cfg.clone()).map_err(|e| {
            OpossumError::Other(format!("Failed to serialize {}: {e}", path.display()))
        })?;

        // Verify round-trip before writing back to disk
        let reparsed: Material = ron::from_str(&serialized).map_err(|e| {
            OpossumError::Other(format!(
                "Round-trip verification failed for {}: {e}",
                path.display()
            ))
        })?;
        assert_eq!(modern, reparsed);

        fs::write(path, format!("{serialized}\n"))
            .map_err(|e| OpossumError::Other(format!("Failed to write {}: {e}", path.display())))?;
        migrated_count += 1;
    }

    println!(
        "Migration complete: {migrated_count} file(s) migrated, {skipped_count} already up-to-date."
    );
    Ok(())
}
