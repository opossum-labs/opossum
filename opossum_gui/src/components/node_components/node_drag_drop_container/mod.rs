use uuid::Uuid;
mod callbacks;
pub mod drag_drop_container;
pub use drag_drop_container::NodeDragDropContainer;

#[derive(Clone, Copy)]
pub struct DraggedNode {
    node_id: Option<Uuid>,
    elem_offset: Option<(f64, f64)>,
}

impl DraggedNode {
    pub fn new(node_id: Option<Uuid>, elem_offset: Option<(f64, f64)>) -> Self {
        Self {
            node_id,
            elem_offset,
        }
    }

    pub fn node_id(&self) -> &Option<Uuid> {
        &self.node_id
    }
    pub fn set_node_id(&mut self, node_id: Uuid) {
        self.node_id = Some(node_id);
    }

    pub fn elem_offset(&self) -> &Option<(f64, f64)> {
        &self.elem_offset
    }
    pub fn set_elem_offset(&mut self, elem_offset: (f64, f64)) {
        self.elem_offset = Some(elem_offset);
    }

    pub fn clear(&mut self) {
        self.node_id = None;
        self.elem_offset = None;
    }
}

#[derive(Clone, Copy)]
pub struct NodeOffset {
    offset: Option<(f64, f64, f64, f64)>,
}

impl NodeOffset {
    pub fn new(offset: Option<(f64, f64, f64, f64)>) -> Self {
        Self { offset }
    }
    pub fn offset(&self) -> &Option<(f64, f64, f64, f64)> {
        &self.offset
    }
    pub fn set_offset(&mut self, offset: Option<(f64, f64, f64, f64)>) {
        self.offset = offset;
    }
}
