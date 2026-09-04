// src/components/scenery_editor/node/node_icon.rs
use dioxus::prelude::*;

#[component]
pub fn NodeSymbolIcon(
    /// ID of the SVG symbol (e.g. "lens", "mirror", "energy meter")
    symbol_id: String,
    /// Optional CSS classes for custom styling or sizing
    #[props(default)]
    class: String,
) -> Element {
    rsx! {
      div { class: "node-symbol-wrapper",
        svg {
          class: "node-symbol-icon {class}",
          // Ensures the symbol scales properly to the container
          // view_box: "0 0 25 25",
          preserve_aspect_ratio: "xMidYMid meet",
          // Reference the symbol defined in the master SVG sprite
          r#use { href: "#{symbol_id}" }
        }
      }
    }
}
