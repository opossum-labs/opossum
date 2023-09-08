use crate::error::OpossumError;
use crate::optic_ports::OpticPorts;

type Result<T> = std::result::Result<T, OpossumError>;

/// This trait deals with the translation of the OpticScenery-graph structure to the dot-file format which is needed to visualize the graphs
pub trait Dottable {
    /// Return component type specific code in 'dot' format for `graphviz` visualization.
    fn to_dot(
        &self,
        node_index: &str,
        name: &str,
        ports: &OpticPorts,
        mut parent_identifier: String,
        inverted: bool,
    ) -> Result<String> {
        let inv_string = if inverted { " (inv)" } else { "" };
        let node_name = format!("{}{}", name, inv_string);
        parent_identifier = if parent_identifier == "" {
            format!("i{}", node_index)
        } else {
            format!("{}_i{}", &parent_identifier, node_index)
        };
        let mut dot_str = format!("\t{} [\n\t\tshape=plaintext\n", &parent_identifier);
        let mut indent_level = 2;
        dot_str.push_str(&self.add_html_like_labels(
            &node_name,
            &mut indent_level,
            ports,
            inverted,
        ));
        Ok(dot_str)
    }

    /// creates a table-cell wrapper around an "inner" string
    fn add_table_cell_container(
        &self,
        inner_str: &str,
        border_flag: bool,
        indent_level: &mut i32,
    ) -> String {
        if inner_str.is_empty() {
            format!(
                "{}<TD BORDER=\"{}\">{}</TD>\n",
                "\t".repeat(*indent_level as usize),
                border_flag,
                inner_str
            )
        } else {
            format!(
                "{}<TD BORDER=\"{}\">{}{}{}</TD>\n",
                "\t".repeat(*indent_level as usize),
                border_flag,
                inner_str,
                "\t".repeat((*indent_level + 1) as usize),
                "\t".repeat(*indent_level as usize)
            )
        }
    }

    /// create the dot-string of each port
    fn create_port_cell_str(
        &self,
        port_name: &str,
        input_flag: bool,
        port_index: usize,
        indent_level: &mut i32,
    ) -> String {
        // inputs marked as green, outputs as blue
        let color_str = if input_flag {
            "\"lightgreen\""
        } else {
            "\"lightblue\""
        };
        // part of the tooltip that describes if the port is an input or output
        let in_out_str = if input_flag {
            "Input port"
        } else {
            "Output port"
        };
        format!(
            "{}<TD PORT=\"{}\" BORDER=\"1\" BGCOLOR={} HREF=\"\" TOOLTIP=\"{} {}: {}\">{}</TD>\n",
            "\t".repeat(*indent_level as usize),
            port_name,
            color_str,
            in_out_str,
            port_index,
            port_name,
            port_index
        )
    }

    /// create the dot-string that describes the ports, including their row/table/cell wrappers
    fn create_port_cells_str(
        &self,
        input_flag: bool,
        indent_level: &mut i32,
        indent_incr: i32,
        ports: &OpticPorts,
    ) -> String {
        let mut ports = if input_flag {
            ports.inputs()
        } else {
            ports.outputs()
        };
        ports.sort();
        let mut dot_str = self.create_html_like_container("row", indent_level, true, 1);

        dot_str.push_str(&self.create_html_like_container("cell", indent_level, true, 1));
        dot_str.push_str(&self.create_html_like_container("table", indent_level, true, 1));
        dot_str.push_str(&self.create_html_like_container("row", indent_level, true, 1));

        dot_str.push_str(&self.add_table_cell_container("", false, indent_level));

        let mut port_index = 1;
        for port in ports {
            dot_str.push_str(&self.create_port_cell_str(
                &port,
                input_flag,
                port_index,
                indent_level,
            ));
            dot_str.push_str(&self.add_table_cell_container("", false, indent_level));
            port_index += 1;
        }
        *indent_level -= 1;

        dot_str.push_str(&self.create_html_like_container("row", indent_level, false, -1));
        dot_str.push_str(&self.create_html_like_container("table", indent_level, false, -1));
        dot_str.push_str(&self.create_html_like_container("cell", indent_level, false, -1));
        dot_str.push_str(&self.create_html_like_container("row", indent_level, false, indent_incr));
        dot_str
    }

    /// Returns the color of the node.
    fn node_color(&self) -> &str {
        "lightgray"
    }

    fn create_main_node_row_str(&self, node_name: &str, indent_level: &mut i32) -> String {
        let mut dot_str = self.create_html_like_container("row", indent_level, true, 1);
        dot_str.push_str(&format!("{}<TD BORDER=\"1\" BGCOLOR=\"{}\" ALIGN=\"CENTER\" WIDTH=\"80\" CELLPADDING=\"10\" HEIGHT=\"80\" STYLE=\"ROUNDED\">{}</TD>\n", "\t".repeat(*indent_level as usize), self.node_color(), node_name));
        *indent_level -= 1;
        dot_str.push_str(&self.create_html_like_container("row", indent_level, false, 0));

        dot_str
    }

    /// starts or ends an html-like container
    fn create_html_like_container(
        &self,
        container_str: &str,
        indent_level: &mut i32,
        start_flag: bool,
        indent_incr: i32,
    ) -> String {
        let container = match container_str {
            "row" => {
                if start_flag {
                    "<TR>"
                } else {
                    "</TR>"
                }
            }
            "cell" => {
                if start_flag {
                    "<TD BORDER=\"0\">"
                } else {
                    "</TD>"
                }
            }
            "table" => {
                if start_flag {
                    "<TABLE BORDER=\"0\" CELLBORDER=\"0\" CELLSPACING=\"0\" CELLPADDING=\"0\" ALIGN=\"CENTER\">"
                } else {
                    "</TABLE>"
                }
            }
            _ => "Invalid container string!",
        };

        let new_str = "\t".repeat(*indent_level as usize) + container + "\n";
        *indent_level += indent_incr;

        new_str
    }

    /// creates the node label defined by html-like strings
    fn add_html_like_labels(
        &self,
        node_name: &str,
        indent_level: &mut i32,
        ports: &OpticPorts,
        inverted: bool,
    ) -> String {
        let mut dot_str = "\t\tlabel=<\n".to_owned();

        // Start Table environment
        dot_str.push_str(&self.create_html_like_container("table", indent_level, true, 1));

        // add row containing the input ports
        dot_str.push_str(&self.create_port_cells_str(!inverted, indent_level, 0, ports));

        // add row containing the node main
        dot_str.push_str(&self.create_main_node_row_str(node_name, indent_level));

        // add row containing the output ports
        dot_str.push_str(&self.create_port_cells_str(inverted, indent_level, -1, ports));

        //end table environment
        dot_str.push_str(&self.create_html_like_container("table", indent_level, false, -1));

        //end node-shape description
        dot_str.push_str(&format!("{}>];\n", "\t".repeat(*indent_level as usize)));
        dot_str
    }
}
