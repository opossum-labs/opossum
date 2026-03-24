#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{api, components::menu_bar::help::update_notifier::UpdateNotifier};
use dioxus::prelude::*;

#[allow(clippy::volatile_composites)]
const LOGO: Asset = asset!("/assets/LogoBanner.svg");

#[component]
pub fn About(mut show_about: Signal<bool>) -> Element {
    let future = use_resource(move || async move { api::get_version().await });
    let about_body = match &*future.read_unchecked() {
        Some(Ok(response)) => rsx! {
            // Invisible table layout for clean alignment of version info
            table { class: "table table-borderless table-sm mb-4 text-start align-middle",
                tbody {
                    // Placeholder for the upcoming GUI version
                    // tr {
                    //     th { class: "text-muted fw-normal ps-0", scope: "row", "GUI Version:" }
                    //     td { class: "text-end pe-0 fw-medium", "v.0.8.0 (coming soon)" }
                    // }
                    tr {
                        th { class: "text-muted fw-normal ps-0", scope: "row", "Opossum Library:" }
                        td { class: "text-end pe-0 fw-medium", "v.{response.opossum_version()}" }
                    }
                    tr {
                        th { class: "text-muted fw-normal ps-0", scope: "row", "Opossum Server:" }
                        td { class: "text-end pe-0 fw-medium", "v.{response.backend_version()}" }
                    }
                }
            }
            // UpdateNotifier component
            div { class: "text-start",
                UpdateNotifier { version_info: response.clone() }
            }
        },
        Some(Err(_)) => rsx! {
            p { class: "text-danger", "Loading about window failed." }
        },
        None => rsx! {
            // Centered loading spinner
            div { class: "d-flex justify-content-center my-4",
                div { class: "spinner-border text-primary", role: "status",
                    span { class: "visually-hidden", "Loading data..." }
                }
            }
        },
    };

    rsx! {
        div {
            class: "modal d-block",
            "tabindex": "-1",
            "data-bs-theme": "light",
            // Semi-transparent dark background overlay
            style: "background-color: rgba(0,0,0,0.6);",
            div { class: "modal-dialog modal-dialog-centered",
                div { class: "modal-content border-0 shadow-lg",
                    // Header without border, just the close button
                    div { class: "modal-header border-bottom-0 pb-0",
                        button {
                            class: "btn-close",
                            "data-bs-dismiss": "modal",
                            onclick: move |_| show_about.set(false),
                        }
                    }
                    div { class: "modal-body pt-0 px-4",
                        // Make the logo larger by using w-100 and removing max-width restrictions
                        img {
                            id: "about-logo",
                            src: LOGO,
                            class: "img-fluid mb-4 w-100 mx-auto d-block",
                        }
                        {about_body}
                    }
                    // Footer without border, centered outline button
                    div { class: "modal-footer border-top-0 pt-0 justify-content-center",
                        button {
                            class: "btn btn-outline-secondary px-4",
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
