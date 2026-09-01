// src/components/scenery_editor/node/node_icon.rs
use dioxus::prelude::*;

#[component]
pub fn NodeSvgIcon(
    /// Raw SVG XML content
    svg_data: &'static str,
    /// Optional CSS classes
    #[props(default)]
    class: String,
) -> Element {
    rsx! {
      div {
        class: "node-icon-wrapper {class}",
        draggable: false,
        // SVG inherits text color via currentColor from this container
        dangerous_inner_html: "{svg_data}",
      }
    }
}
#[component]
pub fn NodeIconSprite(
    /// The ID of the symbol/group in the SVG (e.g. "icon-lens")
    icon_id: &'static str,
    #[props(default)] class: String,
) -> Element {
    rsx! {
      svg { class: "node-icon {class}",
        // Reference the symbol ID defined in the master SVG
        r#use { href: "#{icon_id}" }
      }
    }
}

#[component]
pub fn NodeSymbolIcon(
    /// ID of the SVG symbol (e.g. "lens", "mirror", "energy meter")
    symbol_id: &'static str,
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
