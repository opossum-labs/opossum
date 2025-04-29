pub mod edges;
pub mod graph_editor;
pub mod graph_node;
pub mod node;
pub mod nodes;
pub mod ports;

use dioxus::signals::{GlobalSignal, Signal};
use edges::edges_component::Edges;
pub use graph_editor::{DraggedNode, GraphEditor, NodeOffset};
pub use node::{Node, NodeElement};
pub use nodes::{Nodes, NodesStore};
use opossum_backend::NodeAttr;

pub static NODES_STORE: GlobalSignal<NodesStore> = Signal::global(NodesStore::new);
pub static ACTIVE_NODE: GlobalSignal<Option<NodeAttr>> = Signal::global(|| None::<NodeAttr>);
static EDGES: GlobalSignal<Edges> = Signal::global(Edges::new);
