use crate::{
    optic_node::{Dottable, Optical},
    optic_ports::OpticPorts,
};

#[derive(Debug)]
/// An ideal beamsplitter node with a given splitting ratio.
pub struct NodeBeamSplitter {
    ratio: f32,
}

impl NodeBeamSplitter {
    /// Creates a new [`NodeBeamSplitter`] with a given splitting ratio.
    pub fn new(ratio: f32) -> Self {
        Self { ratio }
    }

    /// Returns the splitting ratio of this [`NodeBeamSplitter`].
    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    /// Sets the splitting ratio of this [`NodeBeamSplitter`].
    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio;
    }
}

impl Default for NodeBeamSplitter {
    /// Create a 50:50 beamsplitter.
    fn default() -> Self {
        Self { ratio: 0.5 }
    }
}
impl Optical for NodeBeamSplitter {
    fn node_type(&self) -> &str {
        "ideal beam splitter"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("input1").unwrap();
        ports.add_input("input2").unwrap();
        ports.add_output("out1_trans1_refl2").unwrap();
        ports.add_output("out2_trans2_refl1").unwrap();
        ports
    }
}

impl Dottable for NodeBeamSplitter {
    fn node_color(&self) -> &str {
        "lightpink"
    }
}
