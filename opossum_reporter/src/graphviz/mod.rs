//! Graphviz DOT and SVG graph generation for optical systems.

pub mod dottable;
pub use dottable::{Dottable, NodeDotRenderer, NodeLayout, format_node_to_dot};

use crate::error::{ReporterError, ReporterResult};
use opossum_core::{
    core_optics::{NodeAttrExt, PortType},
    nodes::NodeGroup,
    properties::proptype::format_quantity,
};
use std::{
    fmt::Write as _,
    fs::{self, File},
    io::Write,
    path::Path,
    process::Stdio,
};
use uom::si::length::meter;
use uuid::Uuid;

/// Returns the Graphviz header for the given top-level scenery.
fn add_dot_header(scenery_name: &str, rankdir: &str) -> String {
    let mut dot_string = String::from("digraph {\n\tfontsize = 10;\n");
    let _ = writeln!(dot_string, "\tcompound = true;");
    let _ = writeln!(dot_string, "\trankdir = \"{rankdir}\";");
    let _ = writeln!(dot_string, "\tlabel=\"{scenery_name}\"");
    let _ = writeln!(dot_string, "\tfontname=\"Courier-monospace\"");
    let _ = writeln!(
        dot_string,
        "\tnode [fontname=\"Courier-monospace\" fontsize = 10]"
    );
    let _ = writeln!(
        dot_string,
        "\tedge [fontname=\"Courier-monospace\" fontsize = 10]\n"
    );
    dot_string
}

/// Recursively resolves the endpoint string (`i<uuid>:<port>`) for a port connection.
fn resolve_port_endpoint(
    group: &NodeGroup,
    node_uuid: Uuid,
    port_name: &str,
) -> ReporterResult<String> {
    let node_idx = group
        .graph()
        .node_idx_by_uuid(node_uuid)
        .ok_or_else(|| ReporterError::Other(format!("Node {} not found in group", node_uuid)))?;
    let node_ref = group.graph().node_by_idx(node_idx)?;
    let node_id = format!("i{}", node_ref.uuid().as_simple());

    if let Some(group_node) = node_ref.as_any().downcast_ref::<NodeGroup>() {
        if group_node.expand_view()? {
            let in_port = group_node.graph().port_map(&PortType::Input).get(port_name);
            let out_port = group_node
                .graph()
                .port_map(&PortType::Output)
                .get(port_name);
            let port_info = if let Some(port) = in_port {
                port
            } else if let Some(port) = out_port {
                port
            } else {
                return Err(ReporterError::Other(format!(
                    "Port {port_name} is not mapped in group {}",
                    group_node.name()
                )));
            };
            return resolve_port_endpoint(group_node, port_info.0, &port_info.1);
        }
    }
    Ok(format!("{node_id}:{port_name}"))
}

/// Writes all nodes and connections of a `NodeGroup` to the DOT output string.
fn write_group_dot(group: &NodeGroup, rankdir: &str, out: &mut String) -> ReporterResult<()> {
    let sorted = group.graph().topologically_sorted()?;
    for idx in &sorted {
        let node_ref = group.graph().node_by_idx(*idx)?;
        let uuid = node_ref.node_attr().uuid().as_simple().to_string();
        let node_name = node_ref.name();
        let inverted = node_ref.inverted();
        let ports = node_ref.ports();

        if let Some(subgroup) = node_ref.as_any().downcast_ref::<NodeGroup>() {
            if subgroup.expand_view()? {
                let inv_str = if inverted { " (inv)" } else { "" };
                let _ = writeln!(
                    out,
                    "  subgraph i{uuid} {{\n\tlabel=\"{node_name}{inv_str}\"\n\tfontsize=8\n\tcluster=true\n\t"
                );
                write_group_dot(subgroup, rankdir, out)?;
                out.push_str("}\n");
                continue;
            }
        }

        let dot_cell = format_node_to_dot(
            &uuid,
            node_name,
            inverted,
            &ports,
            rankdir,
            node_ref.node_color(),
        )?;
        out.push_str(&dot_cell);
    }

    for conn in group.graph().connections() {
        let src_edge_str = resolve_port_endpoint(group, conn.src_id, &conn.src_port)?;
        let target_edge_str = resolve_port_endpoint(group, conn.target_id, &conn.target_port)?;
        let _ = writeln!(
            out,
            "  {src_edge_str} -> {target_edge_str} [label=\"{}\"]",
            format_quantity(meter, conn.distance)
        );
    }
    Ok(())
}

/// Generates a Graphviz DOT representation of the given optical scenery.
///
/// # Errors
/// Returns an error if the graph traversal or DOT string generation fails.
pub fn scenery_to_dot(scenery: &NodeGroup, rankdir: &str) -> ReporterResult<String> {
    let rankdir = if rankdir == "LR" { "LR" } else { "TB" };
    let mut dot_string = add_dot_header(scenery.name(), rankdir);
    write_group_dot(scenery, rankdir, &mut dot_string)?;
    dot_string.push_str("}\n");
    Ok(dot_string)
}

/// Converts a DOT string into an SVG string using the `dot` command-line utility.
///
/// # Errors
/// Returns an error if `dot` is not installed or execution fails.
pub fn dot_to_svg(dot_string: &str) -> ReporterResult<String> {
    let mut child = std::process::Command::new("dot")
        .arg("-Tsvg:cairo")
        .arg("-Kdot")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            ReporterError::Graphviz(format!(
                "conversion to image failed: {e}. Maybe `graphviz` is not installed."
            ))
        })?;

    if let Some(child_stdin) = child.stdin.as_mut() {
        child_stdin
            .write_all(dot_string.as_bytes())
            .map_err(|e| ReporterError::Graphviz(format!("conversion to image failed: {e}")))?;
    } else {
        return Err(ReporterError::Graphviz(
            "could not set stdin for graphviz command".into(),
        ));
    }

    let output = child
        .wait_with_output()
        .map_err(|e| ReporterError::Graphviz(format!("conversion to image failed: {e}")))?;

    let svg_string = String::from_utf8(output.stdout)
        .map_err(|e| ReporterError::Graphviz(format!("conversion to image failed: {e}")))?;
    Ok(svg_string)
}

/// Creates `scenery.dot` and `scenery.svg` in the specified directory.
///
/// # Errors
/// Returns an error if writing files or Graphviz execution fails.
pub fn create_dot_file(scenery: &NodeGroup, output_dir: &Path) -> ReporterResult<()> {
    if !output_dir.exists() {
        fs::create_dir_all(output_dir).map_err(|e| {
            ReporterError::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to create directory '{}': {e}", output_dir.display()),
            ))
        })?;
    }

    let dot_str = scenery_to_dot(scenery, "")?;
    let dot_path = output_dir.join("scenery.dot");
    fs::write(&dot_path, &dot_str).map_err(|e| {
        ReporterError::Io(std::io::Error::new(
            e.kind(),
            format!("writing diagram file (.dot) failed: {e}"),
        ))
    })?;

    let svg_str = dot_to_svg(&dot_str)?;
    let svg_path = output_dir.join("scenery.svg");
    let mut svg_file = File::create(&svg_path).map_err(|e| {
        ReporterError::Io(std::io::Error::new(
            e.kind(),
            format!("writing diagram file (.svg) failed: {e}"),
        ))
    })?;
    write!(svg_file, "{svg_str}").map_err(|e| {
        ReporterError::Io(std::io::Error::new(
            e.kind(),
            format!("writing diagram file (.svg) failed: {e}"),
        ))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_traits::Zero;
    use opossum_core::{
        nodes::{Dummy, EnergyMeter, SourcePort},
        opm_document::OpmDocument,
    };

    #[test]
    fn test_scenery_to_dot() -> ReporterResult<()> {
        let mut scenery = NodeGroup::new("Nested Group Dot test");
        let i_s = scenery.add_node(SourcePort::default())?;
        let mut group = NodeGroup::default();
        group.set_expand_view(true)?;
        let g_n1 = group.add_node(Dummy::new("node1"))?;
        group.map_input_port(g_n1, "input_1", "input_1")?;
        group.map_output_port(g_n1, "output_1", "output_1")?;

        let i_g = scenery.add_node(group)?;
        let i_d = scenery.add_node(EnergyMeter::default())?;

        scenery.connect_nodes(
            i_s,
            "output_1",
            i_g,
            "input_1",
            uom::si::f64::Length::zero(),
        )?;
        scenery.connect_nodes(
            i_g,
            "output_1",
            i_d,
            "input_1",
            uom::si::f64::Length::zero(),
        )?;

        let dot = scenery_to_dot(&scenery, "")?;
        assert!(dot.contains("digraph {"));
        assert!(dot.contains("subgraph i"));
        assert!(dot.contains("Nested Group Dot test"));
        Ok(())
    }

    #[test]
    fn test_create_dot_file() -> ReporterResult<()> {
        let document =
            OpmDocument::from_file(Path::new("./files_for_testing/opm/opticscenery.opm"))?;
        let temp_dir = tempfile::tempdir()?;
        create_dot_file(document.scenery(), temp_dir.path())?;

        let dot_file = temp_dir.path().join("scenery.dot");
        let svg_file = temp_dir.path().join("scenery.svg");
        assert!(dot_file.is_file());
        assert!(svg_file.is_file());
        Ok(())
    }
}
