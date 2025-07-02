#![windows_subsystem = "windows"]
use dioxus::{desktop::tao::window::Icon, prelude::*};
use opossum_gui::App;
use std::io::Cursor;

const MAIN_CSS: Asset = asset!("./assets/main.css");
// const PLOTLY_JS: Asset = asset!("./assets/plotly.js");
// const THREE_MOD_JS: Asset = asset!("./assets/three_mod.js");
// const ORBIT_CTRLS: Asset = asset!("./assets/orbitControls.js");
const MDB_CSS: Asset = asset!("./assets/mdb.min.css");
const MDB_JS: Asset = asset!("./assets/mdb.umd.min.js");
const MDB_SUB_CSS: Asset = asset!("./assets/mdb_submenu.css");
const MDB_ACC_CSS: Asset = asset!("./assets/mdb_accordion.css");

fn read_icon() -> Option<Icon> {
    let icon_bytes: &[u8] = include_bytes!("../../opossum/logo/Logo_square.ico");
    let mut reader = Cursor::new(icon_bytes);
    let icon_dir = ico::IconDir::read(&mut reader).unwrap();
    icon_dir.entries().iter().next().map_or_else(
        || None,
        |entry| {
            let width = entry.width();
            let height = entry.height();
            entry.decode().map_or_else(
                |_| None,
                |image| {
                    let data = image.rgba_data();
                    Icon::from_rgba(data.into(), width, height).ok()
                },
            )
        },
    )
}
fn main() {
    #[cfg(feature = "desktop")]
    fn launch_app() {
        let window = dioxus::desktop::WindowBuilder::new()
            //.with_decorations(true)
            .with_window_icon(read_icon())
            .with_title("Opossum");
        dioxus::LaunchBuilder::new()
            .with_cfg(
                dioxus::desktop::Config::new().with_window(window), //.with_menu(None),
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
        document::Stylesheet { href: MAIN_CSS }
        document::Stylesheet { href: MDB_CSS }
        document::Stylesheet { href: MDB_SUB_CSS }
        document::Stylesheet { href: MDB_ACC_CSS }
        document::Script { src: MDB_JS }
        App {}
    }
}
