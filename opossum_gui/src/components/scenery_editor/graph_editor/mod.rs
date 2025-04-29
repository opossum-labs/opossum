use uuid::Uuid;
mod callbacks;
pub mod graph_editor_component;
pub use graph_editor_component::{GraphEditor, NodeEditorCommand};
mod graph_editor_commands;

#[derive(Clone, Copy)]
pub struct DraggedNode {
    node_id: Option<Uuid>,
    elem_offset: Option<(f64, f64)>,
}

impl DraggedNode {
    #[must_use]
    pub const fn new(node_id: Option<Uuid>, elem_offset: Option<(f64, f64)>) -> Self {
        Self {
            node_id,
            elem_offset,
        }
    }
    #[must_use]
    pub const fn node_id(&self) -> &Option<Uuid> {
        &self.node_id
    }
    pub const fn set_node_id(&mut self, node_id: Uuid) {
        self.node_id = Some(node_id);
    }
    #[must_use]
    pub const fn elem_offset(&self) -> &Option<(f64, f64)> {
        &self.elem_offset
    }
    pub const fn set_elem_offset(&mut self, elem_offset: (f64, f64)) {
        self.elem_offset = Some(elem_offset);
    }
    pub const fn clear(&mut self) {
        self.node_id = None;
        self.elem_offset = None;
    }
}

#[derive(Clone, Copy)]
pub struct NodeOffset {
    offset: Option<(f64, f64, f64, f64)>,
}

impl NodeOffset {
    #[must_use]
    pub const fn new(offset: Option<(f64, f64, f64, f64)>) -> Self {
        Self { offset }
    }
    #[must_use]
    pub const fn offset(&self) -> &Option<(f64, f64, f64, f64)> {
        &self.offset
    }
    pub const fn set_offset(&mut self, offset: Option<(f64, f64, f64, f64)>) {
        self.offset = offset;
    }
}
