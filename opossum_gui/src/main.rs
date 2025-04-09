#![windows_subsystem = "windows"]
use std::rc::Rc;

use dioxus::prelude::*;
use opossum_gui::{
    components::{
        context_menu::cx_menu::ContextMenu, logger::logger_component::Logger,
        menu_bar::menu_bar_component::MenuBar, node_components::NodeDragDropContainer,
        node_property_config::node_config_menu::NodePropertyConfigMenu,
    },
    MainWindowSize, CONTEXT_MENU, MAIN_WINDOW_SIZE,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug)]
struct ApiResponse {
    backend_version: String,
    opossum_version: String,
}

const MAIN_CSS: Asset = asset!(".\\assets\\main.css");
const PLOTLY_JS: Asset = asset!(".\\assets\\plotly.js");
const THREE_MOD_JS: Asset = asset!("./assets/three_mod.js");
const ORBIT_CTRLS: Asset = asset!("./assets/orbitControls.js");
fn main() {
    #[cfg(feature = "desktop")]
    fn launch_app() {
        use dioxus::desktop::{
            tao::{self, platform::windows::IconExtWindows, window::Icon},
            wry::dpi::PhysicalSize,
        };
        let window = tao::window::WindowBuilder::new()
            .with_resizable(true)
            .with_inner_size(PhysicalSize::new(1000, 800))
            .with_background_color((37, 37, 37, 1))
            // .with_decorations(false)
            .with_title("Opossum");

        dioxus::LaunchBuilder::new()
            .with_cfg(
                dioxus::desktop::Config::new()
                    .with_window(window)
                    .with_background_color((37, 37, 37, 1))
                    // .with_menu(None)
                    .with_icon(
                        Icon::from_path(".\\assets\\favicon.ico", None)
                            .expect("Could not load icon"),
                    ),
            )
            .launch(App);
    }

    #[cfg(not(feature = "desktop"))]
    fn launch_app() {
        dioxus::launch(App);
    }

    launch_app();
}

#[component]
fn App() -> Element {
    use_context_provider(|| Signal::new(Uuid::nil()));
    let mut main_window = use_signal(|| None::<Rc<MountedData>>);
    rsx! {
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Script { src: PLOTLY_JS }
        document::Script { r#type: "module", src: ORBIT_CTRLS }
        document::Script { r#type: "module", src: THREE_MOD_JS }
        div {
            class: "col-flex container",
            oncontextmenu: move |evt| {
                evt.prevent_default();
            },
            onclick: move |_| {
                let mut cx_menu = CONTEXT_MENU.write();
                *cx_menu = None;
            },
            ContextMenu { cx_menu: CONTEXT_MENU.read().clone() }
            MenuBar {}
            div {
                class: "row-flex",
                onmounted: move |e| { main_window.set(Some(e.data())) },
                onresize: move |_: Event<ResizeData>| {
                    spawn(async move {
                        if let Some(main_window) = main_window.read().as_ref() {
                            if let Ok(rect) = main_window.get_client_rect().await {
                                let mut main_window_size = MAIN_WINDOW_SIZE.write();
                                *main_window_size = Some(MainWindowSize {
                                    width: rect.size.width,
                                    height: rect.size.height,
                                });
                            }
                        }
                    });
                },
                NodePropertyConfigMenu {}

                div { class: "col-flex",

                    NodeDragDropContainer {}
                    // ThreeJSComponent {  },
                    // PlotComponent {  },
                    Logger {}
                }
            }
        }
    }
}
