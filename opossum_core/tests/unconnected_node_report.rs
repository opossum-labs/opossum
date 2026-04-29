use opossum_core::{
    nodes::{Dummy, NodeGroup},
    reporting::report_note::ReportLevel,
};

#[test]
fn test_unconnected_node_reporting() {
    // 1. Create a NodeGroup
    let mut scenery = NodeGroup::new("test_scenery");

    // 2. Add a connected node (simulated by just adding it, but we need another one to connect to to avoid "single tree" check
    // failing solely on it if we want to distinguish). Actually, a single node is a single tree.
    // If we have two nodes and no connection, we have two trees.
    let node1 = Dummy::new("node1");
    let _uuid1 = scenery.add_node(node1).expect("Failed to add node1");

    let node2 = Dummy::new("node2");
    // Make sure node2 is unconnected
    let _uuid2 = scenery.add_node(node2).expect("Failed to add node2");

    // 3. Run toplevel_report
    // This should trigger the "unconnected sub-trees" warning because we have 2 nodes and 0 edges -> 2 components.
    // And both nodes might be considered stale depending on `is_stale_node` logic (usually checks if it's a source or reachable from one).
    // Dummy nodes are not sources.

    // Let's add a Source node to make it more realistic, but maybe sticking to Dummies is enough to trigger "unconnected".
    let report = scenery.toplevel_report().expect("Analysis failed");

    // 4. Verify Notes
    let notes = report.notes();

    // Check for global warning
    let global_warning_present = notes
        .iter()
        .any(|n| n.level == ReportLevel::Warning && n.message.contains("unconnected sub-trees"));
    assert!(
        global_warning_present,
        "Global warning about unconnected sub-trees missing"
    );

    // Check for specific node warning
    // node2 should certainly be unconnected/stale if it's not connected to anything.
    // node1 as well.

    let node2_warning_present = notes.iter().any(|n| {
        n.level == ReportLevel::Warning && n.message.contains("Node 'node2' is unconnected")
    });
    assert!(
        node2_warning_present,
        "Warning for unconnected node 'node2' missing"
    );
}
