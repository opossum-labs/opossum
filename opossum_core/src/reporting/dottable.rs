#![warn(missing_docs)]
//! # DOT Graph Export
//!
//! Provides traits and implementations for exporting the optical system as a graph
//! in (graphviz) DOT format. Enables visualization and analysis of node connectivity and structure.
//! Types implementing [`Dottable`] can be rendered as graph nodes and edges.
use crate::core_optics::{OpticPorts, PortType};
use crate::error::OpmResult;
use num::ToPrimitive;
use std::fmt::Write as _;

/// Structure for holding the calculated layout parameters for the node's table representation.
pub struct NodeLayout {
    node_cell_size: usize,
    num_cells: usize,
    row_col_span: usize,
    input_port_start: usize,
    output_port_start: usize,
}

/// This trait deals with the translation of the [`NodeGroup`](crate::nodes::NodeGroup) structure to the dot-file
/// format which is needed to visualize the graphs
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
        rankdir: &str,
    ) -> OpmResult<String> {
        let inv_string = if inverted { " (inv)" } else { "" };
        let node_name = format!("{name}{inv_string}");
        let mut dot_str = format!("\ti{node_index} [\n\t\tshape=plaintext\n");
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
            "<TD WIDTH=\"16\" HEIGHT=\"16\" FIXEDSIZE=\"TRUE\" PORT=\"{port_name}\" BORDER=\"1\" BGCOLOR={color_str} HREF=\"\" TOOLTIP=\"{in_out_str} {port_index}: {port_name}\">{port_index}</TD>\n"
        )
    }
    /// Defines the displayed color of the node
    ///
    /// # Returns
    /// Returns the color string of the node.
    fn node_color(&self) -> &'static str {
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

    /// Helper method to calculate layout parameters for the node table.
    fn calculate_layout(
        &self,
        node_name: &str,
        input_port_num: usize,
        output_port_num: usize,
    ) -> NodeLayout {
        let max_ports = input_port_num.max(output_port_num);

        let (num_cells, row_col_span) = if max_ports > 1 {
            ((max_ports + 1) * 2 + 1, max_ports * 2 + 1)
        } else {
            (7, 5)
        };

        let node_name_chars = node_name.len().to_f64().unwrap();
        let min_width_from_ports = 16 * (max_ports * 2 + 1);
        let min_width_from_name = (node_name_chars * 6.5).ceil().to_usize().unwrap();

        let mut single_cell_size = if min_width_from_ports > min_width_from_name {
            (min_width_from_ports + 20) / num_cells
        } else {
            (min_width_from_name + 20) / num_cells
        };

        if single_cell_size < 80 / (num_cells - 2) {
            single_cell_size = 80 / (num_cells - 2);
        }

        let node_cell_size = single_cell_size * (num_cells - 2);

        let input_port_start = if num_cells > 7 || max_ports > 1 {
            max_ports - input_port_num + 2
        } else {
            3
        };

        let output_port_start = if num_cells > 7 || max_ports > 1 {
            max_ports - output_port_num + 2
        } else {
            3
        };

        NodeLayout {
            node_cell_size,
            num_cells,
            row_col_span,
            input_port_start,
            output_port_start,
        }
    }

    /// Helper method to create the main body cell of the node.
    fn create_node_body_cell(&self, node_name: &str, rankdir: &str, layout: &NodeLayout) -> String {
        let (width, height) = if rankdir == "LR" {
            (layout.node_cell_size, layout.node_cell_size)
        } else {
            (layout.node_cell_size, 16 + layout.row_col_span - 1)
        };

        format!(
            "<TD FIXEDSIZE=\"TRUE\" ROWSPAN=\"{}\" COLSPAN=\"{}\" BGCOLOR=\"{}\" WIDTH=\"{}\" HEIGHT=\"{}\" BORDER=\"1\" ALIGN=\"CENTER\" CELLPADDING=\"0\" STYLE=\"ROUNDED\">{}</TD>\n",
            layout.row_col_span,
            layout.row_col_span,
            self.node_color(),
            width,
            height,
            node_name
        )
    }

    /// Helper method to create empty spacer cells.
    fn create_spacer_cell(
        &self,
        ax_nums: (usize, usize),
        rankdir: &str,
        layout: &NodeLayout,
    ) -> String {
        let (row, col) = ax_nums;
        let is_corner =
            (row == 0 || row == layout.num_cells - 1) && (col == 1 || col == layout.num_cells - 2);
        let is_side_middle =
            (col == 0 || col == layout.num_cells - 1) && (row == 1 || row == layout.num_cells - 2);

        if is_corner || is_side_middle {
            let size = (layout.node_cell_size - (layout.num_cells - 4) * 16) / 2;
            let (width, height) = if rankdir == "LR" {
                ("16".to_string(), size.to_string())
            } else {
                (
                    if is_corner {
                        "16".to_string()
                    } else {
                        size.to_string()
                    },
                    "1".to_string(),
                )
            };
            return format!(
                "<TD FIXEDSIZE=\"TRUE\" ALIGN=\"CENTER\" WIDTH=\"{width}\" HEIGHT=\"{height}\"> </TD>\n"
            );
        }

        // Default inner spacer
        let (width, height) = if rankdir == "LR" {
            ("16", "16")
        } else {
            ("16", "1")
        };
        format!(
            "<TD FIXEDSIZE=\"TRUE\" ALIGN=\"CENTER\" WIDTH=\"{width}\" HEIGHT=\"{height}\"> </TD>\n"
        )
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
        rankdir: &str,
    ) -> String {
        let (inputs, outputs) = ports;
        let (in_port_count, out_port_count) = ports_count;
        let (row, col) = ax_nums;

        let layout = self.calculate_layout(node_name, inputs.len(), outputs.len());

        // Try to create an input port cell
        if col == 0
            && *in_port_count < inputs.len()
            && row >= layout.input_port_start
            && (row - layout.input_port_start) % 2 == 0
        {
            let cell = self.create_port_cell_str(&inputs[*in_port_count], true, *in_port_count + 1);
            *in_port_count += 1;
            return cell;
        }
        // Try to create an output port cell
        if col == layout.num_cells - 1
            && *out_port_count < outputs.len()
            && row >= layout.output_port_start
            && (row - layout.output_port_start) % 2 == 0
        {
            let cell =
                self.create_port_cell_str(&outputs[*out_port_count], false, *out_port_count + 1);
            *out_port_count += 1;
            return cell;
        }

        // Try to create the main node body cell
        if row == 1 && col == 1 {
            return self.create_node_body_cell(node_name, rankdir, &layout);
        }
        // Otherwise, create a spacer cell
        self.create_spacer_cell(ax_nums, rankdir, &layout)
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

        let mut inputs = ports.names(&PortType::Input);
        let mut outputs = ports.names(&PortType::Output);
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
                        rankdir,
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
                        rankdir,
                    ));
                }
            }
            *indent_level -= 1;
            dot_str.push_str(&self.create_html_like_container("row", indent_level, false, 0));
        }
        //end table environment
        dot_str.push_str(&self.create_html_like_container("table", indent_level, false, -1));

        //end node-shape description
        let _ = writeln!(dot_str, "{}>];", "\t".repeat(*indent_level));
        dot_str
    }
}

#[cfg(test)]
mod test {
    use crate::{
        error::{OpmResult, OpossumError},
        nodes::{
            BeamSplitter, Dummy, EnergyMeter, Metertype, NodeGroup, SourcePort,
            SplittingConfigBuilder,
        },
    };
    use num::Zero;
    use std::{fs::File, io::Read};
    use uom::si::f64::Length;

    fn get_file_content(f_path: &str) -> OpmResult<String> {
        let file_content = &mut "".to_owned();
        let _ = File::open(f_path)
            .map_err(|e| OpossumError::Other(format!("could not open file: {e}")))?
            .read_to_string(file_content);
        Ok(file_content.to_string())
    }

    #[test]
    fn to_dot_empty() -> OpmResult<()> {
        let file_content_tb = get_file_content("./files_for_testing/dot/to_dot_empty_TB.dot")?;
        let file_content_lr = get_file_content("./files_for_testing/dot/to_dot_empty_LR.dot")?;

        let scenery = NodeGroup::new("Test");

        let scenery_dot_str_tb = scenery.toplevel_dot("TB")?;
        let scenery_dot_str_lr = scenery.toplevel_dot("LR")?;

        assert_eq!(file_content_tb, scenery_dot_str_tb);
        assert_eq!(file_content_lr, scenery_dot_str_lr);
        Ok(())
    }
    #[test]
    #[ignore]
    fn to_dot_with_node() -> OpmResult<()> {
        let file_content_tb = get_file_content("./files_for_testing/dot/to_dot_w_node_TB.dot")?;
        let file_content_lr = get_file_content("./files_for_testing/dot/to_dot_w_node_LR.dot")?;

        let mut scenery = NodeGroup::default();
        scenery.add_node(Dummy::new("Test"))?;
        let scenery_dot_str_tb = scenery.toplevel_dot("TB")?;
        let scenery_dot_str_lr = scenery.toplevel_dot("LR")?;

        assert_eq!(file_content_tb, scenery_dot_str_tb);
        assert_eq!(file_content_lr, scenery_dot_str_lr);
        Ok(())
    }
    #[test]
    #[ignore]
    fn to_dot_full() -> OpmResult<()> {
        let file_content_tb = get_file_content("./files_for_testing/dot/to_dot_full_TB.dot")?;
        let file_content_lr = get_file_content("./files_for_testing/dot/to_dot_full_LR.dot")?;
        let mut scenery = NodeGroup::default();
        let i_s = scenery.add_node(SourcePort::default())?;
        let bs = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6))?;
        let i_bs = scenery.add_node(bs)?;
        let i_d1 = scenery.add_node(EnergyMeter::new(
            "Energy meter 1",
            Metertype::IdealEnergyMeter,
        )?)?;
        let i_d2 = scenery.add_node(EnergyMeter::new(
            "Energy meter 2",
            Metertype::IdealEnergyMeter,
        )?)?;

        scenery.connect_nodes(i_s, "output_1", i_bs, "input_1", Length::zero())?;
        scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_d1, "input_1", Length::zero())?;
        scenery.connect_nodes(i_bs, "out2_trans2_refl1", i_d2, "input_1", Length::zero())?;

        let scenery_dot_str_tb = scenery.toplevel_dot("TB")?;
        let scenery_dot_str_lr = scenery.toplevel_dot("LR")?;

        assert_eq!(file_content_tb, scenery_dot_str_tb);
        assert_eq!(file_content_lr, scenery_dot_str_lr);
        Ok(())
    }
    #[test]
    #[ignore]
    fn to_dot_group() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let mut group1 = NodeGroup::new("group 1");
        group1.set_expand_view(true)?;
        let g1_n1 = group1.add_node(Dummy::new("node1"))?;
        let g1_n2 = group1.add_node(BeamSplitter::default())?;
        group1.map_output_port(g1_n2, "out1_trans1_refl2", "output_1")?;
        group1.connect_nodes(g1_n1, "output_1", g1_n2, "input_1", Length::zero())?;

        let mut nested_group = NodeGroup::new("group 1_1");
        let nested_g_n1 = nested_group.add_node(Dummy::new("node1_1"))?;
        let nested_g_n2 = nested_group.add_node(Dummy::new("node1_2"))?;
        nested_group.set_expand_view(true)?;

        nested_group.connect_nodes(
            nested_g_n1,
            "output_1",
            nested_g_n2,
            "input_1",
            Length::zero(),
        )?;
        nested_group.map_input_port(nested_g_n1, "input_1", "input_1")?;
        nested_group.map_output_port(nested_g_n2, "output_1", "output_1")?;

        let nested_group_id = group1.add_node(nested_group)?;
        group1.connect_nodes(
            nested_group_id,
            "output_1",
            g1_n1,
            "input_1",
            Length::zero(),
        )?;

        let mut group2: NodeGroup = NodeGroup::new("group 2");
        group2.set_expand_view(false)?;
        let g2_n1 = group2.add_node(Dummy::new("node2_1"))?;
        let g2_n2 = group2.add_node(Dummy::new("node2_2"))?;
        group2.map_input_port(g2_n1, "input_1", "input_1")?;

        group2.connect_nodes(g2_n1, "output_1", g2_n2, "input_1", Length::zero())?;

        let scene_g1 = scenery.add_node(group1)?;
        let scene_g2 = scenery.add_node(group2)?;

        // set_output_port
        scenery.connect_nodes(scene_g1, "output_1", scene_g2, "input_1", Length::zero())?;
        let file_content_tb = get_file_content("./files_for_testing/dot/group_dot_TB.dot")?;
        let file_content_lr = get_file_content("./files_for_testing/dot/group_dot_LR.dot")?;
        let scenery_dot_str_tb = scenery.toplevel_dot("TB")?;
        let scenery_dot_str_lr = scenery.toplevel_dot("LR")?;

        assert_eq!(file_content_tb, scenery_dot_str_tb);
        assert_eq!(file_content_lr, scenery_dot_str_lr);
        Ok(())
    }
}
