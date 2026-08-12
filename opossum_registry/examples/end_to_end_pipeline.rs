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
    refractive_index::RefrIndexSellmeier1,
};

// Imports from opossum_registry
use opossum_registry::{AssetIndex, AssetLoader};

fn main() -> OpmResult<()> {
    println!("=== OPOSSUM End-to-End Workflow Demonstration ===\n");

    // -------------------------------------------------------------------------
    // Phase 1: Create local material registry directory and save catalog materials
    // -------------------------------------------------------------------------
    let temp_registry_dir = TempDir::new().expect("Failed to create temp registry dir");
    let loader = AssetLoader::new(temp_registry_dir.path());

    println!("[1/5] Populating local registry on disk...");

    // Define N-BK7 catalog material with standard Sellmeier 1 formula
    let refr_index = RefrIndexSellmeier1::default().into();
    let mut bk7_material = Material::new_draft(
        "N-BK7",
        Some("Schott".to_string()),
        Some("Primary crown glass for optical components".to_string()),
        refr_index,
    );

    // Save material asset into local repository (~/materials/<uuid>/v1.ron)
    let saved_path = loader.publish(&mut bk7_material)?;
    println!("  -> Saved material 'N-BK7' to: {:?}", saved_path);

    // -------------------------------------------------------------------------
    // Phase 2: Index materials in RAM and perform catalog search
    // -------------------------------------------------------------------------
    println!("\n[2/5] Building in-memory index and searching catalog...");
    let mut index = AssetIndex::<Material>::new();
    let item_count = index.build_from_loader(&loader)?;
    println!("  -> Indexed {} material(s) in RAM.", item_count);

    // Perform case-insensitive search
    let search_results = index.search(Some("BK7"), Some("Schott"));
    assert!(
        !search_results.is_empty(),
        "Material search should return N-BK7"
    );

    let found_entry = search_results[0];
    println!(
        "  -> Found catalog item: '{}' by '{}' (UUID: {})",
        found_entry.common.name,
        found_entry
            .common
            .manufacturer
            .as_deref()
            .unwrap_or("Unknown"),
        found_entry.common.id
    );

    // Load full material instance using the UUID resolved from index
    let catalog_material: Material = loader.load(found_entry.common.id, None)?;

    // -------------------------------------------------------------------------
    // Phase 3: Construct optical model in opossum_core using catalog material
    // -------------------------------------------------------------------------
    println!("\n[3/5] Building optical scenery in opossum_core...");
    let mut scenery = NodeGroup::new("Demo Telescope Scenery");

    // Add source port
    let src_id = scenery.add_node(SourcePort::default())?;

    // Add a biconvex lens using the loaded catalog material
    let lens_node = Lens::new(
        "Focusing Lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        catalog_material.clone(),
    )?;
    let lens_id = scenery.add_node(lens_node)?;

    // Add ray visualizer detector
    let detector_node = RayPropagationVisualizer::new("Ray Plot", None)?;
    let detector_id = scenery.add_node(detector_node)?;

    // Connect nodes chronologically: Source -> Lens -> Detector
    scenery.connect_nodes(src_id, "output_1", lens_id, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(
        lens_id,
        "output_1",
        detector_id,
        "input_1",
        millimeter!(90.0),
    )?;

    // Assemble OpmDocument with RayTrace Analyzer configuration
    let mut doc = OpmDocument::new(scenery);
    let mut ray_config = RayTraceConfig::default();
    let ray_builder = round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 5)?;
    ray_config.map_source(src_id, ray_builder);

    doc.add_analyzer(AnalyzerType::RayTrace(ray_config));
    println!("  -> Optical scenery created with 3 nodes and 1 RayTrace analyzer.");

    // -------------------------------------------------------------------------
    // Phase 4: Serialize OpmDocument to .opm File (Vendoring Check)
    // -------------------------------------------------------------------------
    println!("\n[4/6] Serializing model to .opm document file...");
    let temp_opm_file = TempDir::new().expect("Failed to create temp opm dir");
    let opm_file_path = temp_opm_file.path().join("model.opm");

    doc.save_to_file(&opm_file_path)?;
    println!("  -> Document saved to: {:?}", opm_file_path);

    // Read raw string content to verify material vendoring
    let raw_opm_content = std::fs::read_to_string(&opm_file_path).expect("Read OPM file");
    assert!(
        raw_opm_content.contains("embedded_materials:"),
        "OPM file must contain embedded_materials section"
    );
    assert!(
        raw_opm_content.contains("N-BK7"),
        "Embedded materials must store the glass name"
    );
    println!("  -> Verified: Material was automatically embedded in OPM document.");

    // -------------------------------------------------------------------------
    // Phase 5: Reload document, resolve embedded materials, and execute simulation
    // -------------------------------------------------------------------------
    println!("\n[5/6] Reloading .opm file and running optical simulation...");
    let mut reloaded_doc = OpmDocument::from_file(&opm_file_path)?;

    // Run analyzers registered in the document
    let reports = reloaded_doc.analyze()?;
    println!("  -> Simulation finished successfully!");
    println!("  -> Generated {} analysis report(s).", reports.len());

    // -------------------------------------------------------------------------
    // Phase 6: Update existing material (Draft & Publish workflow)
    // -------------------------------------------------------------------------
    println!("\n[6/6] Updating existing material to a new version...");

    // Create a new draft from the existing catalog material (resets version to 0)
    let mut updated_material = catalog_material.new_draft_from();
    assert_eq!(updated_material.version(), 0, "Draft version must be 0");

    // Modify a property of the draft
    updated_material.header.description =
        Some("Updated primary crown glass with expanded metadata".to_string());

    // Publish the updated material. The loader recognizes version 0 and bumps the highest known version (1 -> 2)
    let updated_path = loader.publish(&mut updated_material)?;
    println!(
        "  -> Published updated material 'N-BK7' to: {:?}",
        updated_path
    );
    assert_eq!(
        updated_material.version(),
        2,
        "Published update should be version 2"
    );

    // Rebuild the in-memory index to verify both versions are tracked
    index.build_from_loader(&loader)?;
    let entry_after_update = index
        .get(&bk7_material.id())
        .expect("Material should still be in index");

    println!(
        "  -> Latest version in index: v{}",
        entry_after_update.common.latest_version
    );
    println!(
        "  -> Available versions in index: {:?}",
        entry_after_update.common.available_versions
    );

    assert_eq!(entry_after_update.common.latest_version, 2);
    assert_eq!(entry_after_update.common.available_versions, vec![1, 2]);

    println!("\n=== All pipeline checks passed successfully! ===");
    Ok(())
}
