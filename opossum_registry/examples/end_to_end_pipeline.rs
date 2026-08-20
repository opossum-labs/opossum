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

/// Recursively traverses a directory, writes blobs for all files, and constructs Git tree objects.
///
/// Ignores the `.git` directory and canonicalizes entry sorting according to Git specifications.
fn write_tree_recursive(repo: &gix::Repository, dir: &Path) -> gix::ObjectId {
    let mut entries = Vec::new();
    let dir_entries: Vec<_> = std::fs::read_dir(dir)
        .expect("Failed to read directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() != ".git")
        .collect();

    for entry in dir_entries {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            // Recurse into subdirectories
            let subtree_id = write_tree_recursive(repo, &path);
            entries.push(gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Tree.into(),
                filename: file_name.into(),
                oid: subtree_id,
            });
        } else if path.is_file() {
            // Read file content and write blob object into the Object Database (ODB)
            let data = std::fs::read(&path).expect("Failed to read file");
            let blob_id = repo.write_blob(&data).expect("Failed to write blob").detach();
            entries.push(gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Blob.into(),
                filename: file_name.into(),
                oid: blob_id,
            });
        }
    }

    // Git canonical sort order: directory names are sorted as if ending with '/'
    entries.sort_by(|a, b| {
        let a_name = if a.mode == gix::objs::tree::EntryKind::Tree.into() {
            format!("{}/", a.filename)
        } else {
            a.filename.to_string()
        };
        let b_name = if b.mode == gix::objs::tree::EntryKind::Tree.into() {
            format!("{}/", b.filename)
        } else {
            b.filename.to_string()
        };
        a_name.cmp(&b_name)
    });

    let tree = gix::objs::Tree { entries };
    repo.write_object(&tree).expect("Failed to write tree").detach()
}

/// Helper function to simulate a remote Git repository on the local filesystem.
/// Initializes the repository with a basic README on the `main` branch.
fn setup_dummy_remote(repo_path: &Path) {
    let repo = gix::init(repo_path).expect("Failed to init remote dummy repo");
    let file_path = repo_path.join("README.md");
    std::fs::write(&file_path, "# OPOSSUM Remote Registry\n").expect("Failed to write README.md");

    let tree_oid = write_tree_recursive(&repo, repo_path);

    let commit = gix::objs::Commit {
        tree: tree_oid,
        parents: Default::default(),
        author: gix::actor::Signature {
            name: "OPOSSUM Maintainer".into(),
            email: "admin@opossum.local".into(),
            time: gix::date::Time::now_local_or_utc(),
        },
        committer: gix::actor::Signature {
            name: "OPOSSUM Maintainer".into(),
            email: "admin@opossum.local".into(),
            time: gix::date::Time::now_local_or_utc(),
        },
        encoding: None,
        message: "Initial commit: Registry creation".into(),
        extra_headers: Vec::new(),
    };

    let commit_id = repo.write_object(&commit).expect("Failed to write initial commit").detach();

    // Create refs/heads/main pointing to initial commit
    repo.reference(
        "refs/heads/main",
        commit_id,
        gix::refs::transaction::PreviousValue::Any,
        "Initial commit: Registry creation",
    )
    .expect("Failed to create main branch reference");

    // Ensure HEAD points symbolically to the main branch
    std::fs::write(repo.git_dir().join("HEAD"), "ref: refs/heads/main\n")
        .expect("Failed to point HEAD to main");
}

/// Helper function to simulate a community update on the remote repository.
/// Creates a new material ("F2"), adds it to the tree, and commits it on `main`.
fn simulate_remote_catalog_update(repo_path: &Path) {
    let repo = gix::open(repo_path).expect("Failed to open remote repo");
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

    // Build the updated tree containing the newly published material files
    let tree_oid = write_tree_recursive(&repo, repo_path);

    // Find parent commit on refs/heads/main
    let parent_ref = repo
        .find_reference("refs/heads/main")
        .expect("Failed to find main reference");
    let parent_commit_id = parent_ref
        .into_fully_peeled_id()
        .expect("Failed to peel main ref to commit")
        .detach();

    let commit = gix::objs::Commit {
        tree: tree_oid,
        parents: vec![parent_commit_id].into(),
        author: gix::actor::Signature {
            name: "OPOSSUM Maintainer".into(),
            email: "admin@opossum.local".into(),
            time: gix::date::Time::now_local_or_utc(),
        },
        committer: gix::actor::Signature {
            name: "OPOSSUM Maintainer".into(),
            email: "admin@opossum.local".into(),
            time: gix::date::Time::now_local_or_utc(),
        },
        encoding: None,
        message: "Add F2 material to catalog".into(),
        extra_headers: Vec::new(),
    };

    let commit_id = repo.write_object(&commit).expect("Failed to write catalog commit").detach();

    // Fast-forward refs/heads/main to new commit
    repo.reference(
        "refs/heads/main",
        commit_id,
        gix::refs::transaction::PreviousValue::MustExistAndMatch(parent_commit_id.into()),
        "Add F2 material to catalog",
    )
    .expect("Failed to update main branch reference");
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