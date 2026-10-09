use nalgebra::Point2;
use opossum_core::{core_optics::node_attr::HasNodeAttr, prelude::*};
use uuid::Uuid;
use std::path::Path;

fn main() -> OpmResult<()> {
    // Configuration parameters for the grid layout
    let rows: usize = 25;
    let cols: usize = 40; // 10 x 10 = 100 nodes (change e.g. to 25 x 40 for 1000 nodes)
    let spacing_x: f64 = 100.0;
    let spacing_y: f64 = 50.0;
    let connect_sequentially: bool = false;
    let output_path = Path::new("./opossum_core/playground/many_nodes.opm");

    let mut scenery = NodeGroup::new("Grid Generation Demo");

    // Tracks the previously inserted node ID for sequential port connections
    let mut previous_node_id: Option<Uuid> = None;

    for row in 0..rows {
        for col in 0..cols {
            // Unique identifier for each node
            let node_name = format!("dummy_{}_{}", row, col);
            let mut dummy_node = Dummy::new(&node_name);

            // Compute the GUI canvas position for the current grid coordinate
            let pos_x = col as f64 * spacing_x;
            let pos_y = row as f64 * spacing_y;
            dummy_node
                .node_attr_mut()
                .set_gui_position(Some(Point2::new(pos_x, pos_y)));

            // Add the fully configured node to the scenery
            let current_node_id = scenery.add_node(dummy_node)?;

            // Optionally establish a sequential connection to the previous node
            if connect_sequentially {
                if let Some(prev_id) = previous_node_id {
                    scenery.connect_nodes(
                        prev_id,
                        "output_1",
                        current_node_id,
                        "input_1",
                        millimeter!(0.0),
                    )?;
                }
            }

            previous_node_id = Some(current_node_id);
        }
    }

    // Wrap scenery into an OPM document and attach standard analyzers
    let mut doc = OpmDocument::new(scenery);
    // doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));

    // Ensure the parent directory exists before saving
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            OpossumError::OpmDocument(format!("Failed to create output directory: {err}"))
        })?;
    }

    // Persist to disk
    doc.save_to_file(output_path)?;

    println!(
        "Successfully generated grid with {} nodes at: {:?}",
        rows * cols,
        output_path
    );

    Ok(())
}