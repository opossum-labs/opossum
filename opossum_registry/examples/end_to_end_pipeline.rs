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
use opossum_registry::{
    AssetRegistry, CatalogSource, DEFAULT_BRANCH, OFFICIAL_CATALOG_SUBDIR, sync::RegistrySync,
};
use std::path::Path;
use tempfile::TempDir;

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
            let subtree_id = write_tree_recursive(repo, &path);
            entries.push(gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Tree.into(),
                filename: file_name.into(),
                oid: subtree_id,
            });
        } else if path.is_file() {
            let data = std::fs::read(&path).expect("Failed to read file");
            let blob_id = repo
                .write_blob(&data)
                .expect("Failed to write blob")
                .detach();
            entries.push(gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Blob.into(),
                filename: file_name.into(),
                oid: blob_id,
            });
        }
    }
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
    repo.write_object(&tree)
        .expect("Failed to write tree")
        .detach()
}

/// Initializes a simulated remote Git repository on `refs/heads/main`.
fn setup_dummy_remote(repo_path: &Path) {
    let repo = gix::init(repo_path).expect("Failed to init remote dummy repo");
    let main_ref = format!("refs/heads/{DEFAULT_BRANCH}");
    std::fs::write(repo.git_dir().join("HEAD"), format!("ref: {main_ref}\n"))
        .expect("Failed to point HEAD to main");

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
    let commit_id = repo
        .write_object(&commit)
        .expect("Failed to write initial commit")
        .detach();
    repo.reference(
        main_ref.as_str(),
        commit_id,
        gix::refs::transaction::PreviousValue::Any,
        "Initial commit: Registry creation",
    )
    .expect("Failed to create main branch reference");
}

/// Simulates a remote catalog update adding material "F2" on `main`.
fn simulate_remote_catalog_update(repo_path: &Path) -> OpmResult<()> {
    let repo = gix::open(repo_path).expect("Failed to open remote repo");
    let remote_loader = opossum_registry::loader::AssetLoader::new(repo_path);
    let refr_index = RefrIndexConst::new(1.62)?.into();
    let mut f2_mat = Material::new_draft(
        "F2",
        Some("Schott".to_string()),
        Some("Flint glass".to_string()),
        refr_index,
    );
    remote_loader
        .publish(&mut f2_mat)
        .expect("Failed to publish F2 remotely");

    let tree_oid = write_tree_recursive(&repo, repo_path);
    let main_ref = format!("refs/heads/{DEFAULT_BRANCH}");
    let parent_ref = repo
        .find_reference(main_ref.as_str())
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
    let commit_id = repo
        .write_object(&commit)
        .expect("Failed to write catalog commit")
        .detach();
    repo.reference(
        main_ref.as_str(),
        commit_id,
        gix::refs::transaction::PreviousValue::MustExistAndMatch(parent_commit_id.into()),
        "Add F2 material to catalog",
    )
    .expect("Failed to update main branch reference");
    Ok(())
}

fn main() -> OpmResult<()> {
    println!("=== OPOSSUM Dual-Catalog Workflow Demonstration ===\n");

    println!("[1/7] Setting up dummy remote repository...");
    let temp_remote_dir = TempDir::new().expect("Failed to create temp remote dir");
    setup_dummy_remote(temp_remote_dir.path());
    let remote_url = temp_remote_dir.path().to_string_lossy().to_string();

    println!("\n[2/7] Initializing dual-catalog AssetRegistry (official + user)...");
    let temp_registry_dir = TempDir::new().expect("Failed to create temp registry dir");
    let mut registry = AssetRegistry::<Material>::with_official_remote(
        temp_registry_dir.path(),
        Some(&remote_url),
    )?;

    println!("\n[3/7] Creating custom material in user catalog...");
    let mut bk7_material = Material::new_draft(
        "N-BK7",
        Some("Schott".to_string()),
        Some("User catalog crown glass".to_string()),
        RefrIndexSellmeier1::default().into(),
    );
    let saved_path = registry.publish_to(CatalogSource::User, &mut bk7_material)?;
    println!("  -> Saved user material 'N-BK7' to: {:?}", saved_path);

    println!("\n[4/7] Simulating remote catalog update ('F2')...");
    simulate_remote_catalog_update(temp_remote_dir.path())?;

    println!("\n[5/7] Pulling updates into official catalog and rebuilding index...");
    let official_sync = RegistrySync::new_official(
        temp_registry_dir.path().join(OFFICIAL_CATALOG_SUBDIR),
        Some(&remote_url),
    );
    official_sync.pull_updates()?;
    let total_indexed = registry.rebuild_index()?;
    println!("  -> Indexed {total_indexed} material(s) across official and user catalogs.");

    assert!(!registry.search_text("F2").is_empty());
    let search_local = registry.search_text("BK7");
    assert!(!search_local.is_empty());
    let catalog_material: Material = registry.load(search_local[0].common.id, None)?;

    println!("\n[6/7] Building optical scenery in opossum_core...");
    let mut scenery = NodeGroup::new("Demo Telescope Scenery");
    let src_id = scenery.add_node(SourcePort::default())?;
    let lens_node = Lens::new(
        "Focusing Lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        catalog_material,
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

    println!("\n[7/7] Serializing model to .opm document file...");
    let temp_opm_file = TempDir::new().expect("Failed to create temp opm dir");
    let opm_file_path = temp_opm_file.path().join("model.opm");
    doc.save_to_file(&opm_file_path)?;
    let raw_opm_content = std::fs::read_to_string(&opm_file_path).expect("Read OPM file");
    assert!(raw_opm_content.contains("embedded_materials:"));
    assert!(raw_opm_content.contains("N-BK7"));
    println!("  -> Verified: User material was embedded in OPM document.");

    Ok(())
}
