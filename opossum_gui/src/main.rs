#![windows_subsystem = "windows"]

use dioxus::prelude::*;
use opossum_gui::components::app::App;
// use opossum_gui::router::Route;
use opossum_gui::{
    components::{
        context_menu::cx_menu::ContextMenu, logger::logger_component::Logger,
        menu_bar::menu_bar_component::MenuBar, node_components::NodeDragDropContainer,
        node_property_config::node_config_menu::NodePropertyConfigMenu,
    },
    MainWindowSize, CONTEXT_MENU, MAIN_WINDOW_SIZE,
};

// const MAIN_CSS: Asset = asset!("./assets/main.css");
// const PLOTLY_JS: Asset = asset!("./assets/plotly.js");
// const THREE_MOD_JS: Asset = asset!("./assets/three_mod.js");
// const ORBIT_CTRLS: Asset = asset!("./assets/orbitControls.js");
const MDB_CSS: Asset = asset!("./assets/mdb.min.css");
const MDB_JS: Asset = asset!("./assets/mdb.umd.min.js");
const MDB_SUB_CSS: Asset = asset!("./assets/mdb_submenu.css");
// const BS_CSS: Asset = asset!("./assets/bootstrap.min.css");
// const BS_JS: Asset = asset!("./assets/bootstrap.bundle.min.js");
fn main() {
    #[cfg(feature = "desktop")]
    fn launch_app() {
        use dioxus::desktop::{
            tao::{self, window::Icon},
            wry::dpi::PhysicalSize,
        };
        // let window = tao::window::WindowBuilder::new()
        //     .with_resizable(true)
        //     .with_inner_size(PhysicalSize::new(1000, 800))
        //     //.with_background_color((37, 37, 37, 1))
        //     // .with_decorations(false)
        //     .with_title("Opossum");

        dioxus::LaunchBuilder::new()
            .with_cfg(
                dioxus::desktop::Config::new()
                    //.with_window(window)
                    // .with_menu(None)
                    //  .with_icon(
                    //      Icon::from_path("./assets/favicon.ico", None).expect("Could not load icon"),
                    //  ),
            )
            .launch(app);
    }
    #[cfg(not(feature = "desktop"))]
    fn launch_app() {
        dioxus::launch(app);
    }
    launch_app();
}

#[component]
fn app() -> Element {
    rsx! {
        // document::Stylesheet {href: MAIN_CSS }
        document::Stylesheet { href: MDB_CSS }
        document::Stylesheet { href: MDB_SUB_CSS }
        document::Script { src: MDB_JS }
        div { "data-bs-theme": "dark", class: "d-flex flex-column vh-100", App {} }
    }
}
