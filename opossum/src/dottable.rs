#![warn(missing_docs)]
//! Module handling the export of an [`OpticScenery`](crate::optic_scenery::OpticScenery) into the Graphviz `.dot` format.
use crate::error::OpmResult;
use crate::optic_ports::OpticPorts;

/// This trait deals with the translation of the OpticScenery-graph structure to the dot-file format which is needed to visualize the graphs
pub trait Dottable {
    /// Return component type specific code in 'dot' format for `graphviz` visualization.
    ///
    /// # Errors
    /// This function returns an error if the overridden particular implementation generates an error.
    fn to_dot(
        &self,
        node_index: &str,
        name: &str,
        inverted: bool,
        ports: &OpticPorts,
        mut parent_identifier: String,
        rankdir: &str,
    ) -> OpmResult<String> {
        let inv_string = if inverted { " (inv)" } else { "" };
        let node_name = format!("{name}{inv_string}");
        parent_identifier = if parent_identifier.is_empty() {
            format!("i{node_index}")
        } else {
            format!("{}_i{}", &parent_identifier, node_index)
        };
        let mut dot_str = format!("\t{} [\n\t\tshape=plaintext\n", &parent_identifier);
        let mut indent_level = 2;
        dot_str.push_str(&self.add_html_like_labels(&node_name, &mut indent_level, ports, rankdir));
        Ok(dot_str)
    }
    /// Create the dot-string of each defined port
    ///
    /// # Attributes
    /// * `port_name`:  Name of the port, stored in the table cell
    /// * `input_flag`: Boolean that describes if the port is an input or an output. True for inputs, false for outputs.
    /// * `port_index`: Index of the port for this specific Node
    ///
    /// # Returns
    /// Returns the String that describes the table cell of the ports.
    fn create_port_cell_str(&self, port_name: &str, input_flag: bool, port_index: usize) -> String {
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
            "<TD HEIGHT=\"16\" WIDTH=\"16\" PORT=\"{port_name}\" BORDER=\"1\" BGCOLOR={color_str} HREF=\"\" TOOLTIP=\"{in_out_str} {port_index}: {port_name}\">{port_index}</TD>\n"
        )
    }
    /// Defines the displayed color of the node
    ///
    /// # Returns
    /// Returns the color string of the node.
    fn node_color(&self) -> &str {
        "lightgray"
    }
    /// Creates the start- or end-sequence of an html-like container within the dot file
    ///
    /// # Attributes
    /// * `container_str`:  The inner string that is wrapped in this container
    /// * `indent_level`:   Intendation level when creating the dot file. Just for readability
    /// * `start_flag`:     Boolean that describes if the container starts or ends here. True for sart, false otherwise.
    /// * `indent_incr`:    Defines if the indentation level increases, drops or remains the same. Just for readability.
    ///
    /// # Returns
    /// Returns the String that describes the start- or end-sequence of an html-like container.
    fn create_html_like_container(
        &self,
        container_str: &str,
        indent_level: &mut usize,
        start_flag: bool,
        indent_incr: i32,
    ) -> String {
        let container = match container_str {
            "row" => {
                if start_flag {
                    "<TR BORDER=\"0\">"
                } else {
                    "</TR>"
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

        let new_str = "\t".repeat(*indent_level) + container + "\n";
        *indent_level = indent_level.saturating_add_signed(indent_incr as isize);

        new_str
    }

    /// Creates the respective table cell of the optical node, depending on the number of ports and orientation of the node
    ///
    /// # Attributes
    /// * `ports`:          Reference to the input and output ports
    /// * `ports_count`:    Respective number of input and output ports
    /// * `ax_nums`:        Total number of rows and columns off the node table to display it correctly
    /// * `node_name`:      Name of the node
    ///
    /// # Returns
    /// Returns the String that describes the table cell of the node table.
    fn create_node_table_cells(
        &self,
        ports: (&Vec<String>, &Vec<String>),
        ports_count: (&mut usize, &mut usize),
        ax_nums: (usize, usize),
        node_name: &str,
    ) -> String {
        let mut dot_str = String::new();
        let max_port_num = if ports.0.len() <= ports.1.len() {
            ports.1.len()
        } else {
            ports.0.len()
        };

        let port_0_count = ports_count.0;
        let port_1_count = ports_count.1;

        let (node_cell_size, num_cells, row_col_span, port_0_start, port_1_start) =
            if max_port_num > 1 {
                (
                    16 * (max_port_num * 2 + 1),
                    (max_port_num + 1) * 2 + 1,
                    max_port_num * 2 + 1,
                    max_port_num - ports.0.len() + 2,
                    max_port_num - ports.1.len() + 2,
                )
            } else {
                (80, 7, 5, 3, 3)
            };

        if port_0_count < &mut ports.0.len()
            && ax_nums.0 >= port_0_start
            && (ax_nums.0 - port_0_start) % 2 == 0
            && ax_nums.1 == 0
        {
            dot_str.push_str(&self.create_port_cell_str(
                &ports.0[*port_0_count],
                true,
                *port_0_count + 1,
            ));
            *port_0_count += 1;
        } else if port_1_count < &mut ports.1.len()
            && ax_nums.0 >= port_1_start
            && (ax_nums.0 - port_1_start) % 2 == 0
            && ax_nums.1 == num_cells - 1
        {
            dot_str.push_str(&self.create_port_cell_str(
                &ports.1[*port_1_count],
                false,
                *port_1_count + 1,
            ));
            *port_1_count += 1;
        } else if ax_nums.0 == 1 && ax_nums.1 == 1 {
            dot_str.push_str(&format!(  "<TD ROWSPAN=\"{}\" COLSPAN=\"{}\" BGCOLOR=\"{}\" WIDTH=\"{}\" HEIGHT=\"{}\" BORDER=\"1\" ALIGN=\"CENTER\" CELLPADDING=\"10\" STYLE=\"ROUNDED\">{}</TD>\n", 
                                                row_col_span,
                                                row_col_span,
                                                self.node_color(),
                                                node_cell_size,
                                                node_cell_size,
                                                node_name));
        } else {
            dot_str.push_str("<TD ALIGN=\"CENTER\" HEIGHT=\"16\" WIDTH=\"16\"> </TD>\n");
        };

        dot_str
    }

    /// Creates the html-like label that describes the node to be displayed via graphwiz
    ///
    /// # Attributes
    /// * `node_name`:      Name of the node
    /// * `indent_level`:   Intendation level when creating the dot file. Just for readability
    /// * `ports`:          Reference to the [`OpticPorts`] of the node
    /// * `rankdir`:        Describes the orientation in which the node graph is built. "LR" for left to right or "TB" or "" for top to bottom
    ///
    /// # Returns
    /// Returns the String that describes the complete node in a dot-string label.
    fn add_html_like_labels(
        &self,
        node_name: &str,
        indent_level: &mut usize,
        ports: &OpticPorts,
        rankdir: &str,
    ) -> String {
        let mut dot_str = "\t\tlabel=<\n".to_owned();

        let mut inputs = ports.input_names();
        let mut outputs = ports.output_names();
        let mut in_port_count = 0;
        let mut out_port_count = 0;
        inputs.sort();
        outputs.sort();

        let num_cells = if inputs.len() <= outputs.len() && outputs.len() > 1 {
            (outputs.len() + 1) * 2 + 1
        } else if inputs.len() > outputs.len() && inputs.len() > 1 {
            (inputs.len() + 1) * 2 + 1
        } else {
            7
        };

        // Start Table environment
        dot_str.push_str(&self.create_html_like_container("table", indent_level, true, 1));

        //create each cell of the table
        for row_num in 0..num_cells {
            dot_str.push_str(&self.create_html_like_container("row", indent_level, true, 1));
            for col_num in 0..num_cells {
                if rankdir == "LR"
                    && !(col_num > 1
                        && col_num < num_cells - 1
                        && row_num >= 1
                        && row_num < num_cells - 1)
                    && !(col_num >= 1
                        && col_num < num_cells - 1
                        && row_num > 1
                        && row_num < num_cells - 1)
                {
                    dot_str.push_str(&"\t".repeat(*indent_level));
                    dot_str.push_str(&self.create_node_table_cells(
                        (&inputs, &outputs),
                        (&mut in_port_count, &mut out_port_count),
                        (row_num, col_num),
                        node_name,
                    ));
                } else if rankdir != "LR"
                    && !(row_num > 1
                        && row_num < num_cells - 1
                        && col_num >= 1
                        && col_num < num_cells - 1)
                    && !(row_num >= 1
                        && row_num < num_cells - 1
                        && col_num > 1
                        && col_num < num_cells - 1)
                {
                    dot_str.push_str(&"\t".repeat(*indent_level));
                    dot_str.push_str(&self.create_node_table_cells(
                        (&inputs, &outputs),
                        (&mut in_port_count, &mut out_port_count),
                        (col_num, row_num),
                        node_name,
                    ));
                };
            }
            *indent_level -= 1;
            dot_str.push_str(&self.create_html_like_container("row", indent_level, false, 0));
        }
        //end table environment
        dot_str.push_str(&self.create_html_like_container("table", indent_level, false, -1));

        //end node-shape description
        dot_str.push_str(&format!("{}>];\n", "\t".repeat(*indent_level)));
        dot_str
    }
}
