use crate::HTTP_API_CLIENT;
use dioxus::prelude::*;

const LOGO: Asset = asset!(".\\assets\\LogoBanner.svg");

#[component]
pub fn About(mut show_about: Signal<bool>) -> Element {
    let future = use_resource(move || async move { HTTP_API_CLIENT().get_version().await });

    match &*future.read_unchecked() {
        Some(Ok(response)) => rsx! {
            div { id: "about-window",
                div { id: "about-info",
                    a {
                        id: "about-close",
                        onclick: move |_| show_about.set(false),
                        "🗙"
                    }
                    img { id: "about-logo", src: LOGO }
                    p { {format!("Opossum library: v.{}", response.opossum_version())} }
                    p { {format!("Opossum server: v.{}", response.backend_version())} }
                }
            }
        },
        Some(Err(_)) => rsx! {
            div { id: "about-window",
                div { id: "about-info",
                    a {
                        id: "about-close",
                        onclick: move |_| show_about.set(false),
                        "🗙"
                    }
                    img { id: "about-logo", src: LOGO }
                    p { "Loading about window failed" }
                }
            }
        },
        None => rsx! {
            div { id: "about-window",
                div { id: "about-info",
                    a {
                        id: "about-close",
                        onclick: move |_| show_about.set(false),
                        "🗙"
                    }
                    img { id: "about-logo", src: LOGO }
                    p { "Loading about window" }
                }
            }
        },
    }
}
