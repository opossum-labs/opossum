#![warn(missing_docs)]
//! # Graphviz DOT and SVG Rendering for Node Groups
//!
//! Provides rendering routines for hierarchical groups in expanded and collapsed views.

use super::NodeGroup;
use crate::{
    core_optics::{OpticPorts, PortType},
    error::{OpmResult, OpossumError},
    reporting::Dottable,
};
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process::Stdio,
};

impl NodeGroup {
    /// Returns the expansion flag of this [`NodeGroup`].
    ///   
    /// If true, the group expands and the internal nodes of this group are displayed in the dot format.
    /// If false, only the group node itself is displayed and the internal setup is not shown.
    ///
    /// # Errors
    /// Returns an error if the property "expand view" does not exist or fails to evaluate as boolean.
    pub fn expand_view(&self) -> OpmResult<bool> {
        self.node_attr.get_property_bool("expand view")
    }

    /// Defines whether a [`NodeGroup`] should be displayed expanded or collapsed in the dot diagram.
    ///
    /// # Errors
    /// Returns an error if setting the "expand view" property fails.
    pub fn set_expand_view(&mut self, expand_view: bool) -> OpmResult<()> {
        self.node_attr
            .set_property("expand view", expand_view.into())
    }

    /// Defines and returns the node/port identifier to connect edges in the dot format.
    ///
    /// # Parameters
    /// - `port_name`: External port name of the group.
    /// - `node_id`: String containing the UUID of the parent node.
    ///
    /// # Errors
    /// Returns [`OpossumError::OpticGroup`] if the specified `port_name` is not mapped.
    pub fn get_mapped_port_str(&self, port_name: &str, node_id: &str) -> OpmResult<String> {
        if self.expand_view()? {
            let in_port = self.graph.port_map(&PortType::Input).get(port_name);
            let out_port = self.graph.port_map(&PortType::Output).get(port_name);

            let port_info = if let Some(port) = in_port {
                port
            } else if let Some(port) = out_port {
                port
            } else {
                return Err(OpossumError::OpticGroup(format!(
                    "port {port_name} is not mapped"
                )));
            };
            self.graph.node_idx_by_uuid(port_info.0).map_or_else(
                || Ok(format!("i{}:{}", port_info.0.as_simple(), port_info.1)),
                |node_idx| self.graph().create_node_edge_str(node_idx, &port_info.1),
            )
        } else {
            Ok(format!("{node_id}:{port_name}"))
        }
    }

    /// Creates the dot format of the [`NodeGroup`] in its expanded view.
    fn to_dot_expanded_view(
        &self,
        node_index: &str,
        name: &str,
        inverted: bool,
        rankdir: &str,
    ) -> OpmResult<String> {
        let inv_string = if inverted { "(inv)" } else { "" };
        let mut dot_string = format!(
            "  subgraph i{node_index} {{\n\tlabel=\"{name}{inv_string}\"\n\tfontsize=8\n\tcluster=true\n\t"
        );
        dot_string += &self.graph.create_dot_string(rankdir)?;
        Ok(dot_string)
    }

    /// Creates the dot format of the [`NodeGroup`] in its collapsed view.
    fn to_dot_collapsed_view(
        &self,
        node_index: &str,
        name: &str,
        inverted: bool,
        ports: &OpticPorts,
        rankdir: &str,
    ) -> String {
        let inv_string = if inverted { " (inv)" } else { "" };
        let node_name = format!("{name}{inv_string}");
        let mut dot_str = format!("\ti{node_index} [\n\t\tshape=plaintext\n");
        let mut indent_level = 2;
        dot_str.push_str(&self.add_html_like_labels(&node_name, &mut indent_level, ports, rankdir));
        dot_str
    }

    /// Returns the dot-file header of this [`NodeGroup`] graph.
    fn add_dot_header(&self, rankdir: &str) -> String {
        use std::fmt::Write;
        let mut dot_string = String::from("digraph {\n\tfontsize = 10;\n");
        let _ = writeln!(dot_string, "\tcompound = true;");
        let _ = writeln!(dot_string, "\trankdir = \"{rankdir}\";");
        let _ = writeln!(dot_string, "\tlabel=\"{}\"", self.node_attr.name());
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

    /// Export the optic graph, including ports, into the `dot` format.
    ///
    /// # Errors
    /// Returns an error if node names or graphviz structure rendering fails.
    pub fn toplevel_dot(&self, rankdir: &str) -> OpmResult<String> {
        let mut dot_string = self.add_dot_header(rankdir);
        dot_string += &self.graph.create_dot_string(rankdir)?;
        Ok(dot_string)
    }

    /// Generate an SVG file from the top-level [`NodeGroup`] dot diagram.
    ///
    /// # Errors
    /// Returns an error if reading the dot file, child process execution, or writing the SVG fails.
    pub fn toplevel_dot_svg(&self, dot_str_file: &PathBuf, svg_file: &mut File) -> OpmResult<()> {
        let dot_string = fs::read_to_string(dot_str_file)
            .map_err(|e| OpossumError::Other(format!("writing diagram file (.svg) failed: {e}")))?;
        let svg_str = Self::dot_string_to_svg_str(dot_string.as_str())?;
        write!(svg_file, "{svg_str}")
            .map_err(|e| OpossumError::Other(format!("writing diagram file (.svg) failed: {e}")))
    }

    fn dot_string_to_svg_str(dot_string: &str) -> OpmResult<String> {
        let mut child = std::process::Command::new("dot")
            .arg("-Tsvg:cairo")
            .arg("-Kdot")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                OpossumError::Other(format!(
                    "conversion to image failed: {e}. Maybe `graphviz` is not installed."
                ))
            })?;

        let Some(child_stdin) = child.stdin.as_mut() else {
            return Err(OpossumError::Other(
                "conversion to image failed: could not set stdin for graphviz command".into(),
            ));
        };
        child_stdin
            .write_all(dot_string.as_bytes())
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;

        let output = child
            .wait_with_output()
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;

        let svg_string = String::from_utf8(output.stdout)
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;
        Ok(svg_string)
    }
}

impl Dottable for NodeGroup {
    fn to_dot(
        &self,
        node_index: &str,
        name: &str,
        inverted: bool,
        ports: &OpticPorts,
        rankdir: &str,
    ) -> OpmResult<String> {
        let mut cloned_group = self.clone();
        if self.node_attr.inverted() {
            cloned_group.graph.invert_graph()?;
        }
        let dot_str = if self.expand_view()? {
            cloned_group.to_dot_expanded_view(node_index, name, inverted, rankdir)
        } else {
            Ok(cloned_group.to_dot_collapsed_view(node_index, name, inverted, ports, rankdir))
        };
        if self.node_attr.inverted() {
            cloned_group.graph.invert_graph()?;
        }
        dot_str
    }

    fn node_color(&self) -> &'static str {
        "yellow"
    }
}
