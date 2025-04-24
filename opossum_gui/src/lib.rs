use api::http_client::HTTPClient;
use components::{
    context_menu::cx_menu::CxMenu,
    logger::Logs,
    node_components::{
        edges::edges_component::Edges, node_drag_drop_container::drag_drop_container::Zoom,
        NodesStore,
    },
};
use dioxus::signals::{GlobalSignal, Signal};
use opossum_backend::NodeAttr;

pub mod api;
pub mod components;
pub mod router;

pub struct MainWindowSize {
    pub width: f64,
    pub height: f64,
}

static OPOSSUM_UI_LOGS: GlobalSignal<Logs> = Signal::global(Logs::new);
static HTTP_API_CLIENT: GlobalSignal<HTTPClient> = Signal::global(HTTPClient::new);
static EDGES: GlobalSignal<Edges> = Signal::global(Edges::new);
pub static NODES_STORE: GlobalSignal<NodesStore> = Signal::global(NodesStore::new);
static ZOOM: GlobalSignal<Zoom> = Signal::global(|| Zoom::new(1., 1.));
pub static MAIN_WINDOW_SIZE: GlobalSignal<Option<MainWindowSize>> =
    Signal::global(|| None::<MainWindowSize>);
pub static CONTEXT_MENU: GlobalSignal<Option<CxMenu>> = Signal::global(|| None::<CxMenu>);
pub static ACTIVE_NODE: GlobalSignal<Option<NodeAttr>> = Signal::global(|| None::<NodeAttr>);
