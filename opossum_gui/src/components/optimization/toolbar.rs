use crate::components::primitives::button::{Button, ButtonSize, ButtonVariant};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaFolderOpen, FaFloppyDisk, FaWandMagicSparkles},
};

#[component]
pub fn OptimizationToolbar() -> Element {
    rsx! {
      div { class: "row g-2 align-items-center mb-3 p-2 bg-dark rounded border border-secondary",
        div { class: "col-auto d-flex align-items-center gap-2",
          span { class: "text-muted small fw-bold d-flex align-items-center gap-1",
            Icon { icon: FaWandMagicSparkles }
            "Workflow Presets:"
          }
          div { class: "btn-group btn-group-sm",
            button { class: "btn btn-outline-secondary btn-sm", "Kepler Telescope" }
            button { class: "btn btn-outline-secondary btn-sm active", "Focus Spot Minimizer" }
            button { class: "btn btn-outline-secondary btn-sm", "Beam Collimator" }
          }
        }
        div { class: "col d-flex justify-content-end gap-2",
          Button {
            variant: ButtonVariant::Outline,
            size: ButtonSize::Sm,
            title: "Load .opt.json configuration",
            Icon { icon: FaFolderOpen }
            "Load Config"
          }
          Button {
            variant: ButtonVariant::Outline,
            size: ButtonSize::Sm,
            title: "Export configuration to .opt.json",
            Icon { icon: FaFloppyDisk }
            "Save Preset"
          }
        }
      }
    }
}