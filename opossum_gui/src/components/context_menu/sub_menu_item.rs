use dioxus::prelude::*;

#[component]
pub fn MenuItem(
    class: String,
    onclick: Option<Callback<Event<MouseData>>>,
    display: String,
) -> Element {
    rsx! {
        a {
            class,
            onclick: move |e| {
                if let Some(on_click_fn) = onclick {
                    on_click_fn(e);
                }
            },
            {display}
        }
    }
}
