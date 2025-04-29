use api::http_client::HTTPClient;
use components::{
    context_menu::cx_menu::CxMenu, logger::Logs,
    scenery_editor::graph_editor::graph_editor_component::Zoom,
};
use dioxus::signals::{GlobalSignal, Signal};

pub mod api;
pub mod components;

pub struct MainWindowSize {
    pub width: f64,
    pub height: f64,
}

static OPOSSUM_UI_LOGS: GlobalSignal<Logs> = Signal::global(Logs::new);
static HTTP_API_CLIENT: GlobalSignal<HTTPClient> = Signal::global(HTTPClient::new);
static ZOOM: GlobalSignal<Zoom> = Signal::global(|| Zoom::new(1., 1.));
pub static MAIN_WINDOW_SIZE: GlobalSignal<Option<MainWindowSize>> =
    Signal::global(|| None::<MainWindowSize>);
pub static CONTEXT_MENU: GlobalSignal<Option<CxMenu>> = Signal::global(|| None::<CxMenu>);
