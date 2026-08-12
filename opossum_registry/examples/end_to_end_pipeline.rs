use git2::IndexAddOption;
use std::path::Path;
use tempfile::TempDir;

// Imports from opossum_core
use opossum_core::{
    analyzers::{AnalyzerType, raytrace::RayTraceConfig},
    error::OpmResult,
    joule,
    material::Material,
    millimeter,
    nodes::{Lens, NodeGroup, RayPropagationVisualizer, SourcePort, round_collimated_ray_builder},
    opm_document::OpmDocument,
    refractive_index::{RefrIndexConst, RefrIndexSellmeier1},
};

// Imports from opossum_registry
use opossum_registry::{AssetIndex, AssetLoader, sync::RegistrySync};

/// Helper function to simulate a remote Git repository on the local filesystem.
/// Initializes the repo with a basic README.
fn setup_dummy_remote(repo_path: &Path) {
    let repo = git2::Repository::init(repo_path).expect("Failed to init remote dummy repo");
    let file_path = repo_path.join("README.md");
    std::fs::write(&file_path, "# OPOSSUM Remote Registry\n").unwrap();

    let mut index = repo.index().unwrap();
    index.add_path(Path::new("README.md")).unwrap();
    let oid = index.write_tree().unwrap();
    let signature = git2::Signature::now("OPOSSUM Maintainer", "admin@opossum.local").unwrap();
    let tree = repo.find_tree(oid).unwrap();

    // Commit directly to the `main` branch
    repo.commit(
        Some("refs/heads/main"),
        &signature,
        &signature,
        "Initial commit: Registry creation",
        &tree,
        &[],
    )
    .unwrap();

    // Ensure HEAD points to the newly created main branch, not the system default (e.g., master)
    repo.set_head("refs/heads/main").unwrap();
}

/// Helper function to simulate a community update on the remote repository.
/// Creates a new material ("F2"), adds it to the index, and commits it.
fn simulate_remote_catalog_update(repo_path: &Path) {
    let repo = git2::Repository::open(repo_path).expect("Failed to open remote repo");
    let loader = AssetLoader::new(repo_path);

    // Create new material on the "server"
    let refr_index = RefrIndexConst::new(1.62).unwrap().into();
    let mut f2_mat = Material::new_draft(
        "F2",
        Some("Schott".to_string()),
        Some("Flint glass".to_string()),
        refr_index,
    );
    loader
        .publish(&mut f2_mat)
        .expect("Failed to publish F2 remotely");

    // Commit the generated .ron files explicitly via the "materials" folder pathspec
    let mut index = repo.index().unwrap();
    index
        .add_all(["materials"].iter(), IndexAddOption::DEFAULT, None)
        .unwrap();
    let oid = index.write_tree().unwrap();
    let signature = git2::Signature::now("OPOSSUM Maintainer", "admin@opossum.local").unwrap();
    let tree = repo.find_tree(oid).unwrap();

    // Safely get the parent commit directly from the main branch instead of relying on HEAD
    let parent_commit = repo
        .find_reference("refs/heads/main")
        .unwrap()
        .peel_to_commit()
        .unwrap();

    repo.commit(
        Some("refs/heads/main"),
        &signature,
        &signature,
        "Add F2 material to catalog",
        &tree,
        &[&parent_commit],
    )
    .unwrap();
}

fn main() -> OpmResult<()> {
    println!("=== OPOSSUM End-to-End Workflow Demonstration ===\n");

    // -------------------------------------------------------------------------
    // Phase 0: Setup a local "Remote" Git Repository (Simulating GitHub/GitLab)
    // -------------------------------------------------------------------------
    println!("[1/8] Setting up dummy remote repository...");
    let temp_remote_dir = TempDir::new().expect("Failed to create temp remote dir");
    setup_dummy_remote(temp_remote_dir.path());
    let remote_url = temp_remote_dir.path().to_string_lossy().to_string();
    println!("  -> Remote server simulated at: {}", remote_url);

    // -------------------------------------------------------------------------
    // Phase 1: Clone registry from "Remote" and initialize AssetLoader
    // -------------------------------------------------------------------------
    println!("\n[2/8] Cloning registry via RegistrySync...");
    let temp_registry_dir = TempDir::new().expect("Failed to create temp registry dir");

    let sync = RegistrySync::new(temp_registry_dir.path(), &remote_url);
    sync.init_or_clone()?;

    let loader = AssetLoader::new(temp_registry_dir.path());
    println!("  -> Cloned successfully into local working directory.");

    // -------------------------------------------------------------------------
    // Phase 2: Create a local material WITHOUT committing it
    // -------------------------------------------------------------------------
    println!("\n[3/8] Creating local custom material (untracked by Git)...");

    let refr_index = RefrIndexSellmeier1::default().into();
    let mut bk7_material = Material::new_draft(
        "N-BK7",
        Some("Schott".to_string()),
        Some("Local custom crown glass".to_string()),
        refr_index,
    );

    let saved_path = loader.publish(&mut bk7_material)?;
    println!("  -> Saved local material 'N-BK7' to: {:?}", saved_path);

    // -------------------------------------------------------------------------
    // Phase 3: Simulate community adding a material to the remote repository
    // -------------------------------------------------------------------------
    println!("\n[4/8] Simulating community update on the remote server...");
    simulate_remote_catalog_update(temp_remote_dir.path());
    println!("  -> Remote repository has been updated with material 'F2'.");

    // -------------------------------------------------------------------------
    // Phase 4: Pull updates from the remote repository
    // -------------------------------------------------------------------------
    println!("\n[5/8] Pulling updates from remote repository...");
    sync.pull_updates()?;
    println!("  -> Successfully pulled changes. Local repository is up to date.");

    // -------------------------------------------------------------------------
    // Phase 5: Index materials in RAM and verify both materials exist
    // -------------------------------------------------------------------------
    println!("\n[6/8] Building in-memory index and verifying availability...");
    let mut index = AssetIndex::<Material>::new();
    let item_count = index.build_from_loader(&loader)?;
    println!("  -> Indexed {} material(s) in RAM.", item_count);

    // Verify remote material
    let search_remote = index.search(Some("F2"), None);
    assert!(
        !search_remote.is_empty(),
        "Material search should return F2 from remote"
    );
    println!(
        "  -> Found remote catalog item: '{}'",
        search_remote[0].common.name
    );

    // Verify local material
    let search_local = index.search(Some("BK7"), None);
    assert!(
        !search_local.is_empty(),
        "Material search should return N-BK7 from local"
    );
    println!(
        "  -> Found local custom item:   '{}'",
        search_local[0].common.name
    );

    let catalog_material: Material = loader.load(search_local[0].common.id, None)?;

    // -------------------------------------------------------------------------
    // Phase 6: Construct optical model in opossum_core using the local material
    // -------------------------------------------------------------------------
    println!("\n[7/8] Building optical scenery in opossum_core...");
    let mut scenery = NodeGroup::new("Demo Telescope Scenery");

    let src_id = scenery.add_node(SourcePort::default())?;
    let lens_node = Lens::new(
        "Focusing Lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        catalog_material.clone(),
    )?;
    let lens_id = scenery.add_node(lens_node)?;
    let detector_node = RayPropagationVisualizer::new("Ray Plot", None)?;
    let detector_id = scenery.add_node(detector_node)?;

    scenery.connect_nodes(src_id, "output_1", lens_id, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(
        lens_id,
        "output_1",
        detector_id,
        "input_1",
        millimeter!(90.0),
    )?;

    let mut doc = OpmDocument::new(scenery);
    let mut ray_config = RayTraceConfig::default();
    let ray_builder = round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 5)?;
    ray_config.map_source(src_id, ray_builder);
    doc.add_analyzer(AnalyzerType::RayTrace(ray_config));
    println!("  -> Optical scenery created with 3 nodes.");

    // -------------------------------------------------------------------------
    // Phase 7: Serialize OpmDocument to .opm File (Vendoring Check)
    // -------------------------------------------------------------------------
    println!("\n[8/8] Serializing model to .opm document file...");
    let temp_opm_file = TempDir::new().expect("Failed to create temp opm dir");
    let opm_file_path = temp_opm_file.path().join("model.opm");

    doc.save_to_file(&opm_file_path)?;
    println!("  -> Document saved to: {:?}", opm_file_path);

    let raw_opm_content = std::fs::read_to_string(&opm_file_path).expect("Read OPM file");
    assert!(
        raw_opm_content.contains("embedded_materials:"),
        "OPM file must contain embedded_materials section"
    );
    assert!(
        raw_opm_content.contains("N-BK7"),
        "Embedded materials must store the glass name"
    );
    println!("  -> Verified: Local Material was automatically embedded in OPM document.");

    println!("\n=== All pipeline checks passed successfully! ===");
    Ok(())
}
