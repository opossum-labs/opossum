#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{api, HTTP_API_CLIENT};
use dioxus::prelude::*;

const LOGO: Asset = asset!("./assets/LogoBanner.svg");

#[component]
pub fn About(mut show_about: Signal<bool>) -> Element {
    let future = use_resource(move || async move { api::get_version(&HTTP_API_CLIENT()).await });
    let about_body=match &*future.read_unchecked() {
        Some(Ok(response)) => rsx!{
            p { {format!("Opossum library: v.{}", response.opossum_version())} }
            p { {format!("Opossum server: v.{}", response.backend_version())} }
        },
        Some(Err(_)) => rsx!{
            p { "Loading about window failed" }
        },
        None => rsx!{
            p { "Loading data..." }
        }
    };
    rsx!{
        div {
            class: "modal d-block",
            "tabindex": "-1",
            "data-bs-theme": "light",
            div { class: "modal-dialog modal-dialog-centered",
                div { class: "modal-content",
                    div { class: "modal-header",
                        h5 { class: "modal-title", "OPOSSUM" }
                        button {
                            class: "btn-close",
                            "data-bs-dismiss": "modal",
                            onclick: move |_| show_about.set(false),
                        }
                    }
                    div { class: "modal-body",
                        img { id: "about-logo", src: LOGO }
                        {about_body}
                    }
                    div { class: "modal-footer",
                        button {
                            class: "btn btn-secondary",
                            "data-bs-dismiss": "modal",
                            onclick: move |_| show_about.set(false),
                            "Close"
                        }
                    }
                }
            }
        }
    }
}
