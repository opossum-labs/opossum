use api::http_client::HTTPClient;
use components::{context_menu::cx_menu::CxMenu, logger::Logs};
use dioxus::signals::{GlobalSignal, Signal};

mod api;
mod components;

pub use components::app::App;

static OPOSSUM_UI_LOGS: GlobalSignal<Logs> = Signal::global(Logs::new);
pub static HTTP_API_CLIENT: GlobalSignal<HTTPClient> = Signal::global(HTTPClient::new);
static CONTEXT_MENU: GlobalSignal<Option<CxMenu>> = Signal::global(|| None::<CxMenu>);
