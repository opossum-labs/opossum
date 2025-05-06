mod callbacks;
pub mod graph_editor_component;
pub use graph_editor_component::{GraphEditor, NodeEditorCommand};
mod graph_editor_commands;

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
