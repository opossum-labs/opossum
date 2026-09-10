// src/platform/web.rs

use dioxus::prelude::*;

/// Public platform launch entry point for browser / WebAssembly targets.
pub fn launch(root_component: fn() -> Element) {
    dioxus::logger::init(dioxus::logger::tracing::Level::INFO).ok();
    dioxus::launch(root_component);
}

/// No-op lifecycle wrapper for web targets (no local backend process to terminate).
#[component]
pub fn PlatformLifecycle(children: Element) -> Element {
    rsx! {
      {children}
    }
}
