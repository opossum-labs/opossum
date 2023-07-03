use std::{fmt::Debug, rc::Rc};

use crate::optic_ports::OpticPorts;
/// An [`OpticNode`] is the basic struct representing an optical component.
pub struct OpticNode {
    name: String,
    node: Box<dyn OpticComponent>,
    ports: OpticPorts
}

impl OpticNode {
    /// Creates a new [`OpticNode`]. The concrete type of the component must be given while using the `new` function.
    /// The node type ist a struct implementing the [`Optical`] trait. Since the size of the node type is not known at compile time it must be added as `Box<nodetype>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use opossum::optic_node::OpticNode;
    /// use opossum::nodes::NodeDummy;
    ///
    /// let node=OpticNode::new("My node", NodeDummy);
    /// ```
    pub fn new<T: OpticComponent+ 'static>(name: &str, node_type: T) -> Self {
        let ports=node_type.ports();
        Self {
            name: name.into(),
            node: Box::new(node_type),
            ports
        }
    }
    /// Sets the name of this [`OpticNode`].
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
    /// Returns a reference to the name of this [`OpticNode`].
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Returns a string representation of the [`OpticNode`] in `graphviz` format including port visualization. 
    /// This function is normally called by the top-level `to_dot`function within `OpticScenery`.
    pub fn to_dot(&self, node_index: &str) -> String{
        self.node.to_dot(node_index, &self.name, self.inverted(), &self.node.ports())
    }

    /// Returns the concrete node type as string representation.
    pub fn node_type(&self) -> &str {
        self.node.node_type()
    }
    /// Mark the [`OpticNode`] as inverted.
    ///
    /// This means that the node is used in "reverse" direction. All output port become input parts and vice versa.
    pub fn set_inverted(&mut self, inverted: bool) {
        self.ports.set_inverted(inverted)
    }
    /// Returns if the [`OpticNode`] is used in reversed direction.
    pub fn inverted(&self) -> bool {
        self.ports.inverted()
    }
    /// Returns a reference to the [`OpticPorts`] of this [`OpticNode`].
    pub fn ports(&self) -> &OpticPorts {
        &self.ports
    }
}

impl Debug for OpticNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {:?}", self.name, self.node)
    }
}

/// This trait must be implemented by all concrete optical components.
pub trait Optical {
    /// Return the type of the optical component (lens, filter, ...). The default implementation returns "undefined".
    fn node_type(&self) -> &str {
        "undefined"
    }

    fn ports(&self) -> OpticPorts {
        OpticPorts::default()
    }
}

//this trait deals with the translation of the OpticScenery-graph structure to the dot-file format which is needed to visualize the graphs
pub trait Dottable {
    /// Return component type specific code in 'dot' format for `graphviz` visualization.
    fn to_dot(&self, node_index: &str, name: &str, inverted: bool, ports: &OpticPorts) -> String{
        let inv_string = if inverted { " (inv)" } else { "" };
        let node_name = format!("{}{}", name, inv_string);        
        let mut dot_str = format!("\ti{} [\n\t\tshape=plaintext\n", node_index);
        let mut indent_level = 2;
        dot_str.push_str(&self.add_html_like_labels(&node_name, &mut indent_level, ports, inverted));
        dot_str
    }

    // creates a table-cell wrapper around an "inner" string
    fn add_table_cell_container(&self, inner_str: &str, border_flag: bool, indent_level: &mut i32) -> String {
        if inner_str =="" {
            format!("{}<TD BORDER=\"{}\">{}</TD>\n", "\t".repeat(*indent_level as usize), border_flag, inner_str)
        }
        else{
            format!("{}<TD BORDER=\"{}\">{}{}{}</TD>\n", 
            "\t".repeat(*indent_level as usize), border_flag, 
            inner_str, "\t".repeat((*indent_level+1) as usize ), "\t".repeat(*indent_level as usize))
        }
    }

    /// create the dot-string of each port
    fn create_port_cell_str(&self, port_name: &str, input_flag:bool, port_index: usize, indent_level: &mut i32) -> String {
        // inputs marked as green, outputs as blue
        let color_str = if input_flag {"\"lightgreen\""} else {"\"lightblue\""};
        // part of the tooltip that describes if the port is an input or output
        let in_out_str = if input_flag {"Input port"} else {"Output port"};
        format!("{}<TD PORT=\"{}\" BORDER=\"1\" BGCOLOR={} HREF=\"\" TOOLTIP=\"{} {}: {}\">{}</TD>\n", 
                "\t".repeat(*indent_level as usize),
                port_name, color_str, 
                in_out_str, port_index, 
                port_name, port_index)
    }   

    /// create the dot-string that describes the ports, including their row/table/cell wrappers
    fn create_port_cells_str(&self, input_flag:bool, indent_level: &mut i32, indent_incr: i32, ports: &OpticPorts) -> String{
        let mut ports = if input_flag {ports.inputs()} else {ports.outputs()};
        ports.sort();
        let mut dot_str = self.create_html_like_container("row",   indent_level, true, 1);

        dot_str.push_str(&self.create_html_like_container("cell",  indent_level, true, 1));
        dot_str.push_str(&self.create_html_like_container("table", indent_level, true, 1));
        dot_str.push_str(&self.create_html_like_container("row",   indent_level, true, 1));

        dot_str.push_str(&self.add_table_cell_container("", false, indent_level));

        let mut port_index = 1;
        for port in ports{
            dot_str.push_str(&self.create_port_cell_str(&port, input_flag, port_index, indent_level));
            dot_str.push_str(&self.add_table_cell_container("", false, indent_level));
            port_index += 1;
        };
        *indent_level -= 1;

        dot_str.push_str(&self.create_html_like_container("row",   indent_level, false,-1));
        dot_str.push_str(&self.create_html_like_container("table", indent_level, false,-1));
        dot_str.push_str(&self.create_html_like_container("cell",  indent_level, false,-1));
        dot_str.push_str(&self.create_html_like_container("row",   indent_level, false,indent_incr));
        dot_str
    }

    fn node_color(&self) -> &str {
        "lightgray"
      }

    fn create_main_node_row_str(&self, node_name: &str, indent_level: &mut i32)->String {
        let mut dot_str = self.create_html_like_container("row",   indent_level, true, 1);
        dot_str.push_str(&format!("{}<TD BORDER=\"1\" BGCOLOR=\"{}\" ALIGN=\"CENTER\" WIDTH=\"80\" CELLPADDING=\"10\" HEIGHT=\"80\" STYLE=\"ROUNDED\">{}</TD>\n", "\t".repeat(*indent_level as usize), self.node_color(), node_name));
        *indent_level -= 1;
        dot_str.push_str(&self.create_html_like_container("row",   indent_level, false, 0));

        dot_str
    }

    /// starts or ends an html-like container
    fn create_html_like_container(&self, container_str: &str, indent_level: &mut i32, start_flag:bool, indent_incr: i32) -> String{
        let container = match container_str{
            "row"       => if start_flag{"<TR>"} else {"</TR>"},
            "cell"      => if start_flag{"<TD BORDER=\"0\">"} else {"</TD>"},
            "table"     => if start_flag{"<TABLE BORDER=\"0\" CELLBORDER=\"0\" CELLSPACING=\"0\" CELLPADDING=\"0\" ALIGN=\"CENTER\">"} else {"</TABLE>"},
            _           => "Invalid container string!",
        };

        let new_str = "\t".repeat(*indent_level as usize) + container + "\n";
        *indent_level += indent_incr;

        new_str
    }

    /// creates the node label defined by html-like strings
    fn add_html_like_labels(&self, node_name: &str, indent_level: &mut i32, ports: &OpticPorts, inverted: bool) -> String{
        let mut dot_str = "\t\tlabel=<\n".to_owned();

        // Start Table environment
        dot_str.push_str(&self.create_html_like_container("table", indent_level, true, 1));

        // add row containing the input ports
        dot_str.push_str(&self.create_port_cells_str(if inverted {false} else {true}, indent_level, 0, ports));

        // add row containing the node main
        dot_str.push_str(&self.create_main_node_row_str(node_name, indent_level));        

        // add row containing the output ports
        dot_str.push_str(&self.create_port_cells_str(if inverted {true} else {false}, indent_level, -1, ports));

        //end table environment
        dot_str.push_str(&self.create_html_like_container("table", indent_level, false, -1));

        //end node-shape description
        dot_str.push_str(&format!("{}>];\n","\t".repeat(*indent_level as usize) ));
        dot_str

    }
}

pub trait OpticComponent: Optical + Dottable + Debug {}
impl<T: Optical + Dottable + Debug> OpticComponent for T {}


#[cfg(test)]
mod test {
    use super::OpticNode;
    use crate::nodes::NodeDummy;
    #[test]
    fn new() {
        let node = OpticNode::new("Test", NodeDummy);
        assert_eq!(node.name, "Test");
        assert_eq!(node.inverted(), false);
    }
    #[test]
    fn set_name() {
        let mut node = OpticNode::new("Test", NodeDummy);
        node.set_name("Test2".into());
        assert_eq!(node.name, "Test2")
    }
    #[test]
    fn name() {
        let node = OpticNode::new("Test", NodeDummy);
        assert_eq!(node.name(), "Test")
    }
    #[test]
    fn set_inverted() {
        let mut node = OpticNode::new("Test", NodeDummy);
        node.set_inverted(true);
        assert_eq!(node.inverted(), true)
    }
    #[test]
    fn inverted() {
        let mut node = OpticNode::new("Test", NodeDummy);
        node.set_inverted(true);
        assert_eq!(node.inverted(), true)
    }
    #[test]
    fn to_dot() {
        let node = OpticNode::new("Test", NodeDummy);
        assert_eq!(node.to_dot("i0"), "  i0 [label=\"Test\"]\n".to_owned())
    }
    #[test]
    fn to_dot_inverted() {
        let mut node = OpticNode::new("Test", NodeDummy);
        node.set_inverted(true);
        assert_eq!(node.to_dot("i0"), "  i0 [label=\"Test(inv)\"]\n".to_owned())
    }
    #[test]
    fn node_type() {
        let node = OpticNode::new("Test", NodeDummy);
        assert_eq!(node.node_type(), "dummy");
    }
}
