pub mod edges;
pub mod graph_editor;
pub mod graph_node;
pub mod node;
pub mod nodes;
pub mod ports;

use dioxus::signals::{GlobalSignal, Signal};
use edges::edges_component::Edges;
pub use graph_editor::GraphEditor;
static EDGES: GlobalSignal<Edges> = Signal::global(Edges::new);
