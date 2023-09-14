use crate::analyzer::AnalyzerType;
use crate::error::OpossumError;
use crate::lightdata::LightData;
use crate::optic_ports::OpticPorts;
use core::fmt::Debug;
use std::any::Any;
use std::collections::HashMap;

pub type LightResult = HashMap<String, Option<LightData>>;
type Result<T> = std::result::Result<T, OpossumError>;

/// An [`OpticNode`] is the basic struct representing an optical component.
pub struct OpticNode {
    name: String,
    node: Box<dyn OpticComponent>,
    ports: OpticPorts,
}

impl OpticNode {
    /// Creates a new [`OpticNode`]. The concrete type of the component must be given while using the `new` function.
    /// The node type ist a struct implementing the [`Optical`] trait. Since the size of the node type is not known at compile time it must be added as `Box<nodetype>`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use opossum::optic_node::OpticNode;
    /// use opossum::nodes::Dummy;
    ///
    /// let node=OpticNode::new("My node", Dummy::default());
    /// ```
    pub fn new<T: OpticComponent + 'static>(name: &str, node_type: T) -> Self {
        let ports = node_type.ports();
        Self {
            name: name.into(),
            node: Box::new(node_type),
            ports,
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
    pub fn to_dot(&self, node_index: &str, parent_identifier: String, rankdir: &str) -> Result<String> {
        self.node.to_dot(
            node_index,
            &self.name,
            self.inverted(),
            &self.node.ports(),
            parent_identifier,
            rankdir, 
        )
    }

    /// Returns the concrete node type as string representation.
    pub fn node_type(&self) -> &str {
        self.node.node_type()
    }
    /// Mark the [`OpticNode`] as inverted.
    ///
    /// This means that the node is used in "reverse" direction. All output port become input parts and vice versa.
    pub fn set_inverted(&mut self, inverted: bool) {
        self.ports.set_inverted(inverted);
        self.node.set_inverted(inverted);
    }
    /// Returns if the [`OpticNode`] is used in reversed direction.
    pub fn inverted(&self) -> bool {
        self.ports.inverted()
    }
    /// Returns a reference to the [`OpticPorts`] of this [`OpticNode`].
    pub fn ports(&self) -> &OpticPorts {
        &self.ports
    }
    pub fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> Result<LightResult> {
        self.node.analyze(incoming_data, analyzer_type)
    }
    pub fn export_data(&self) {
        let file_name = self.name.to_owned() + ".svg";
        self.node.export_data(&file_name);
    }
    pub fn node(&self) -> &Box<dyn OpticComponent> {
        &self.node
    }
    pub fn is_detector(&self) -> bool {
        self.node.is_detector()
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
    /// Return the available (input & output) ports of this element.
    fn ports(&self) -> OpticPorts {
        OpticPorts::default()
    }
    /// Perform an analysis of this element. The type of analysis is given by an [`AnalyzerType`].
    ///
    /// This function is normally only called by [`OpticScenery::analyze()`](crate::optic_scenery::OpticScenery::analyze()).
    ///
    /// # Errors
    ///
    /// This function will return an error if internal element-specific errors occur and the analysis cannot be performed.
    fn analyze(
        &mut self,
        _incoming_data: LightResult,
        _analyzer_type: &AnalyzerType,
    ) -> Result<LightResult> {
        print!("{}: No analyze function defined.", self.node_type());
        Ok(LightResult::default())
    }
    fn export_data(&self, _file_name: &str) {
        println!("no export_data function implemented for nodetype <{}>", self.node_type())
    }
    fn is_detector(&self) -> bool {
        false
    }
    fn set_inverted(&mut self, _inverted: bool) {}
}

/// This trait deals with the translation of the OpticScenery-graph structure to the dot-file format which is needed to visualize the graphs
pub trait Dottable {
    /// Return component type specific code in 'dot' format for `graphviz` visualization.
    fn to_dot(
        &self,
        node_index: &str,
        name: &str,
        inverted: bool,
        ports: &OpticPorts,
        mut parent_identifier: String,
        rankdir: &str
    ) -> Result<String> {
        let inv_string = if inverted { " (inv)" } else { "" };
        let node_name = format!("{}{}", name, inv_string);
        parent_identifier = if parent_identifier.is_empty() {
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
            rankdir
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
            "<TD HEIGHT=\"16\" WIDTH=\"16\" PORT=\"{}\" BORDER=\"1\" BGCOLOR={} HREF=\"\" TOOLTIP=\"{} {}: {}\">{}</TD>\n",
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
                    "<TR BORDER=\"0\">"
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

    fn create_node_table_cells(
        &self, 
        ports:         (&Vec<String>,&Vec<String>), 
        // outputs:        &Vec<String>,
        ports_count:    (&mut usize, &mut usize),
        ax_nums:        (usize,usize),
        node_name:      &str
    ) -> String{

        let mut dot_str = "".to_owned();
        let max_port_num = if ports.0.len() <= ports.1.len(){
            ports.1.len()
        } else{
            ports.0.len()
        };


        let port_0_count = ports_count.0;
        let port_1_count = ports_count.1;
        
    
        let (node_cell_size, 
            num_cells, 
            row_col_span,
            port_0_start, 
            port_1_start
        ) = if max_port_num > 1{
            (16*(max_port_num*2+1), 
            (max_port_num+1)*2+1, 
            max_port_num*2+1, 
            max_port_num - ports.0.len()+2, 
            max_port_num - ports.1.len()+2
            )
        }
        else {
            (80, 7, 5, 3,3)
        };
        
        if port_0_count < &mut ports.0.len() && ax_nums.0 >= port_0_start && (ax_nums.0-port_0_start) % 2 == 0 && ax_nums.1 == 0 {
            dot_str.push_str(&self.create_port_cell_str(
                &ports.0[*port_0_count],
                true,
                *port_0_count+1,
            ));
            *port_0_count+=1;
        }
        else if port_1_count < &mut ports.1.len() &&  ax_nums.0 >= port_1_start && (ax_nums.0-port_1_start) % 2 == 0 && ax_nums.1 == num_cells-1{
            dot_str.push_str(&self.create_port_cell_str(
                &ports.1[*port_1_count],
                false,
                *port_1_count+1,
            ));
            *port_1_count+=1;
        }
        else if ax_nums.0 == 1 && ax_nums.1 == 1{
            dot_str.push_str(&format!(  "<TD ROWSPAN=\"{}\" COLSPAN=\"{}\" BGCOLOR=\"{}\" WIDTH=\"{}\" HEIGHT=\"{}\" BORDER=\"1\" ALIGN=\"CENTER\" CELLPADDING=\"10\" STYLE=\"ROUNDED\">{}</TD>\n", 
                                                row_col_span, 
                                                row_col_span, 
                                                self.node_color(), 
                                                node_cell_size, 
                                                node_cell_size, 
                                                node_name));
        }
        else{
            dot_str.push_str("<TD ALIGN=\"CENTER\" HEIGHT=\"16\" WIDTH=\"16\"> </TD>\n")
        };

        dot_str
    }

    /// creates the node label defined by html-like strings
    fn add_html_like_labels(
        &self,
        node_name: &str,
        indent_level: &mut i32,
        ports: &OpticPorts,
        inverted: bool,
        rankdir:&str,
    ) -> String {
        let mut dot_str = "\t\tlabel=<\n".to_owned();

        
        let mut inputs = ports.inputs();
        let mut outputs = ports.outputs();
        let mut in_port_count = 0;
        let mut out_port_count = 0;
        inputs.sort();
        outputs.sort();
        
        let num_cells = if inputs.len() <=outputs.len() &&  outputs.len() > 1{
            (outputs.len()+1)*2+1
        } else if inputs.len() > outputs.len() &&  inputs.len() > 1{
            (inputs.len()+1)*2+1
        }
        else{
            7
        };

        let port_list = if inverted{(&outputs, &inputs)}else{(&inputs, &outputs)};


        // Start Table environment
        dot_str.push_str(&self.create_html_like_container("table", indent_level, true, 1));

        //create each cell of the table
        for row_num in 0..num_cells{
            dot_str.push_str(&self.create_html_like_container("row", indent_level, true, 1));
            for col_num in 0..num_cells{
                if rankdir == "LR" 
                   && !(col_num > 1 && col_num <num_cells-1 && row_num>= 1 && row_num <num_cells-1) 
                   && !(col_num >= 1 && col_num <num_cells-1 && row_num> 1 && row_num <num_cells-1){

                    dot_str.push_str(&"\t".repeat(*indent_level as usize));
                    dot_str.push_str(
                        &self.create_node_table_cells(
                            port_list,
                            (&mut in_port_count, &mut out_port_count),
                            (row_num,col_num),
                            node_name
                        )
                    )
                }
                else if rankdir != "LR" 
                        && !(row_num > 1 && row_num <num_cells-1 && col_num>= 1 && col_num <num_cells-1)
                        && !(row_num >= 1 && row_num <num_cells-1 && col_num> 1 && col_num <num_cells-1){
                    dot_str.push_str(&"\t".repeat(*indent_level as usize));
                    dot_str.push_str(
                        &self.create_node_table_cells(
                        port_list,                        
                        (&mut in_port_count, &mut out_port_count),
                        (col_num, row_num),
                        node_name)
                    )
                };
            }
            *indent_level -= 1;
            dot_str.push_str(&self.create_html_like_container("row", indent_level, false, 0));

        }
        //end table environment
        dot_str.push_str(&self.create_html_like_container("table", indent_level, false, -1));

        //end node-shape description
        dot_str.push_str(&format!("{}>];\n", "\t".repeat(*indent_level as usize)));
        dot_str
    }
}

pub trait OpticComponent: Optical + Dottable + Debug + Any + 'static {
    fn upcast_any_ref(&'_ self) -> &'_ dyn Any;
}
impl<T: Optical + Dottable + Debug + Any + 'static> OpticComponent for T {
    #[inline]
    fn upcast_any_ref(&'_ self) -> &'_ dyn Any {
        self
    }
}

impl dyn OpticComponent + 'static {
    #[inline]
    pub fn downcast_ref<T: 'static>(&'_ self) -> Option<&'_ T> {
        self.upcast_any_ref().downcast_ref::<T>()
    }
}

#[cfg(test)]
mod test {
    use super::OpticNode;
    use crate::nodes::{Detector, Dummy, BeamSplitter, NodeGroup};
    use std::{fs::File,io::Read};

    #[test]
    fn new() {
        let node = OpticNode::new("Test", Dummy::default());
        assert_eq!(node.name, "Test");
        assert_eq!(node.inverted(), false);
    }
    #[test]
    fn set_name() {
        let mut node = OpticNode::new("Test", Dummy::default());
        node.set_name("Test2".into());
        assert_eq!(node.name, "Test2")
    }
    #[test]
    fn name() {
        let node = OpticNode::new("Test", Dummy::default());
        assert_eq!(node.name(), "Test")
    }
    #[test]
    fn set_inverted() {
        let mut node = OpticNode::new("Test", Dummy::default());
        node.set_inverted(true);
        assert_eq!(node.inverted(), true)
    }
    #[test]
    fn inverted() {
        let mut node = OpticNode::new("Test", Dummy::default());
        node.set_inverted(true);
        assert_eq!(node.inverted(), true)
    }
    #[test]
    fn is_detector() {
        let node = OpticNode::new("Test", Dummy::default());
        assert_eq!(node.is_detector(), false);
        let node = OpticNode::new("Test", Detector::default());
        assert_eq!(node.is_detector(), true)
    }
    #[test]
    fn to_dot(){
        let path = "files_for_testing/dot/to_dot_single_node_TB.dot";
        let file_content_tb = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_tb);

        let path = "files_for_testing/dot/to_dot_single_node_LR.dot";
        let file_content_lr = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_lr);

        let node = OpticNode::new("Test", Dummy::default());
        let node_dot_str_lr = node.to_dot(
            "0", 
            "".to_owned(), 
            "LR"
        ).unwrap(); 

        let node_dot_str_tb = node.to_dot(
            "0", 
            "".to_owned(), 
            "LR"
        ).unwrap(); 

        assert_eq!(file_content_tb.clone(), node_dot_str_tb);
        assert_eq!(file_content_lr.clone(), node_dot_str_lr);

    }
    #[test]
    fn to_dot_inverted() {
        let path = "files_for_testing/dot/to_dot_single_node_inverted_TB.dot";
        let file_content_tb = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_tb);

        let path = "files_for_testing/dot/to_dot_single_node_inverted_LR.dot";
        let file_content_lr = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_lr);

        let mut node = OpticNode::new("Test", Dummy::default());
        node.set_inverted(true);
        let node_dot_str_lr = node.to_dot(
            "0", 
            "".to_owned(), 
            "LR"
        ).unwrap(); 

        let node_dot_str_tb = node.to_dot(
            "0", 
            "".to_owned(), 
            "LR"
        ).unwrap(); 

        assert_eq!(file_content_tb.clone(), node_dot_str_tb);
        assert_eq!(file_content_lr.clone(), node_dot_str_lr);
    }
    #[test]
    fn to_dot_group(){
        let path = "files_for_testing/dot/to_dot_group_node_expand_TB.dot";
        let file_content_expand_tb = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_expand_tb);

        let path = "files_for_testing/dot/to_dot_group_node_collapse_TB.dot";
        let file_content_collapse_tb = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_collapse_tb);

        let path = "files_for_testing/dot/to_dot_group_node_expand_LR.dot";
        let file_content_expand_lr = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_expand_lr);

        let path = "files_for_testing/dot/to_dot_group_node_collapse_LR.dot";
        let file_content_collapse_lr = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_collapse_lr);

        let mut group1 = NodeGroup::new();
        group1.expand_view(true);
        let g1_n1 = group1.add_node(OpticNode::new("TFP1_g1", Dummy::default()));
        let g1_n2 = group1.add_node(OpticNode::new("TFP2_g1", BeamSplitter::default()));
        group1.map_output_port(g1_n2, "out1_trans1_refl2", "out1").unwrap();
        group1.connect_nodes(g1_n1, "rear", g1_n2, "input1").unwrap();

        let group_node = OpticNode::new("Group1_TFPs", group1.clone());
        let node_dot_str_expand_lr = group_node.to_dot(
            "0", 
            "".to_owned(), 
            "LR").unwrap();
        let node_dot_str_expand_tb = group_node.to_dot(
            "0", 
            "".to_owned(), 
            "LR").unwrap();

        group1.expand_view(false);

        let group_node = OpticNode::new("Group1_TFPs", group1);
        let node_dot_str_collapse_lr = group_node.to_dot(
            "0", 
            "".to_owned(), 
            "LR").unwrap();
        let node_dot_str_collapse_tb = group_node.to_dot(
            "0", 
            "".to_owned(), 
            "LR").unwrap();

        assert_eq!(file_content_expand_lr.clone(), node_dot_str_expand_lr);
        assert_eq!(file_content_expand_tb.clone(), node_dot_str_expand_tb);
        assert_eq!(file_content_collapse_lr.clone(), node_dot_str_collapse_lr);
        assert_eq!(file_content_collapse_tb.clone(), node_dot_str_collapse_tb);
    }
    #[test]
    fn node_type() {
        let node = OpticNode::new("Test", Dummy::default());
        assert_eq!(node.node_type(), "dummy");
    }
}
