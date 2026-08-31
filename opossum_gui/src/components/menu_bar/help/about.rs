use crate::{api, components::menu_bar::help::update_notifier::UpdateNotifier};
use dioxus::prelude::*;

#[allow(clippy::volatile_composites)]
const LOGO: Asset = asset!("/assets/LogoBanner.svg");

#[component]
pub fn About(mut show_about: Signal<bool>) -> Element {
    let future = use_resource(move || async move { api::get_version().await });
    let about_body = match &*future.read_unchecked() {
        Some(Ok(response)) => rsx! {
            table { class: "table table-borderless table-sm mb-4 text-start align-middle",
                tbody {
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
            div { class: "text-start",
                UpdateNotifier { version_info: response.clone() }
            }
        },
        Some(Err(_)) => rsx! {
            div { class: "alert alert-warning text-start mb-4", role: "alert",
                h6 { class: "alert-heading fw-bold mb-1", "No Connection to Backend" }
                p { class: "mb-0 small",
                    "Could not connect to the OPOSSUM backend server. Please verify that the backend is running and reachable."
                }
            }
        },
        None => rsx! {
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
            style: "background-color: rgba(0,0,0,0.6);",
            div { class: "modal-dialog modal-dialog-centered",
                div { class: "modal-content border-0 shadow-lg",
                    div { class: "modal-header border-bottom-0 pb-0",
                        button {
                            class: "btn-close",
                            "data-bs-dismiss": "modal",
                            onclick: move |_| show_about.set(false),
                        }
                    }
                    div { class: "modal-body pt-0 px-4",
                        img {
                            id: "about-logo",
                            src: LOGO,
                            class: "img-fluid mb-4 w-100 mx-auto d-block",
                        }
                        {about_body}
                    }
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
