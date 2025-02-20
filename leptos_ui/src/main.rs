use leptos_ui::App;
use log::debug;

fn main() {
    console_log::init_with_level(log::Level::Debug).expect("error initializing log");
    leptos::mount::mount_to_body(App);
    debug!("Leptos App gestartet!");
}
