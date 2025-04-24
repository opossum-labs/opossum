use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct NewNode {
    node_type: String,
    gui_position: (i32, i32, i32),
}
impl NewNode {
    #[must_use]
    pub const fn new(node_type: String, gui_position: (i32, i32, i32)) -> Self {
        Self {
            node_type,
            gui_position,
        }
    }
}
