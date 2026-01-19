use dioxus::prelude::*;

#[component]
pub fn MenuItem(
    // Optional, falls du mal keine spezielle Klasse brauchst
    #[props(default = "context-menu-item".to_string())] class: String,
    // EventHandler ist oft angenehmer als Option<Callback>
    onclick: EventHandler<MouseEvent>,
    // Erlaubt Icons oder formatierten Text statt nur String
    children: Element,
) -> Element {
    rsx! {
        a { class: "{class}", onclick: move |e| onclick.call(e), {children} }
    }
}
