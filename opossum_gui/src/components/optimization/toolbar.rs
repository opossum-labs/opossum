use crate::components::primitives::button::{Button, ButtonSize, ButtonVariant};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaCube, FaFloppyDisk, FaFolderOpen, FaWandMagicSparkles},
};

#[component]
pub fn OptimizationToolbar() -> Element {
    rsx! {
      div { class: "row g-2 align-items-center mb-3 p-2 bg-dark rounded border border-secondary",
        // 1. Subsystem / Scope Selector
        div { class: "col-auto d-flex align-items-center gap-2",
          span { class: "text-muted small fw-bold d-flex align-items-center gap-1",
            Icon { icon: FaCube }
            "Active Optimization Scope:"
          }
          select {
            class: "form-select form-select-sm bg-black text-light border-secondary",
            style: "width: 17rem;",
            option { selected: true, "Entire Laser Chain (32 Variables, 18 Criteria)" }
            option { "Spatial Filter 1 only (4 Variables, 3 Criteria)" }
            option { "Spatial Filter 2 only (3 Variables, 2 Criteria)" }
            option { "Main Amplifier Stage only (8 Variables, 4 Criteria)" }
          }
        }

        // 2. Rule-Based Assistants
        div { class: "col-auto d-flex align-items-center gap-1",
          div { class: "dropdown",
            button {
              class: "btn btn-outline-info btn-sm dropdown-toggle d-flex align-items-center gap-1",
              r#type: "button",
              "data-bs-toggle": "dropdown",
              Icon { icon: FaWandMagicSparkles }
              "Rule Assistants"
            }
            ul { class: "dropdown-menu dropdown-menu-dark shadow small",
              li {
                a { class: "dropdown-item", href: "#",
                  "Auto-protect all Lenses (Min Edge >= 1.5 mm)"
                }
              }
              li {
                a { class: "dropdown-item", href: "#",
                  "Add Default Box Bounds (±20% on current values)"
                }
              }
              li {
                a { class: "dropdown-item", href: "#",
                  "Auto-Instrument: Insert virtual wavefront checkpoints"
                }
              }
              li {
                hr { class: "dropdown-divider" }
              }
              li {
                a {
                  class: "dropdown-item text-danger",
                  href: "#",
                  "Reset all bounds and couplings"
                }
              }
            }
          }
        }

        // 3. Template Storage
        div { class: "col d-flex justify-content-end gap-2",
          Button {
            variant: ButtonVariant::Outline,
            size: ButtonSize::Sm,
            title: "Load .opt.json configuration profile",
            Icon { icon: FaFolderOpen }
            "Load Profile"
          }
          Button {
            variant: ButtonVariant::Outline,
            size: ButtonSize::Sm,
            title: "Save active profile to .opt.json",
            Icon { icon: FaFloppyDisk }
            "Save Profile"
          }
        }
      }
    }
}
