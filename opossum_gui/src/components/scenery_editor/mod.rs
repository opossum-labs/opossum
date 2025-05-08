pub mod graph_editor;

mod edges;
mod graph_store;
mod node;
mod nodes;
mod ports;

pub use graph_editor::GraphEditor;

use dioxus::signals::{GlobalSignal, Signal};

// static EDGES: GlobalSignal<Edges> = Signal::global(Edges::new);
