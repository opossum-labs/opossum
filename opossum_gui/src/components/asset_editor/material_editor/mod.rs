pub mod optical_properties_editor;

use dioxus::prelude::*;
use opossum_core::material::Material;
use crate::components::primitives::{alert_dialog::{AlertDialog, AlertDialogAction, AlertDialogActions, AlertDialogCancel, AlertDialogDescription, AlertDialogTitle}, scroll_area::ScrollArea};

use super::asset_header_editor::{
    AssetHeaderChangeAction, AssetHeaderChangeEvent, AssetHeaderEditor,
};
use optical_properties_editor::{
    OpticalPropertiesChangeAction, OpticalPropertiesChangeEvent, OpticalPropertiesEditor,
};

/// Actions representing modifications to a Material.
#[derive(Debug, Clone, PartialEq)]
pub enum MaterialChangeAction {
    /// Modifications inside the `AssetHeader` (Name, Manufacturer, Description).
    Header(AssetHeaderChangeAction),
    /// Modifications inside the `OpticalProperties` (Dispersion model, Absorption).
    Optical(OpticalPropertiesChangeAction),
}
impl MaterialChangeAction {
    /// Applies the change action directly to the given `Material`
    /// by delegating to the specific sub-actions.
    pub fn apply(self, material: &mut opossum_core::material::Material) {
        match self {
            Self::Header(header_action) => header_action.apply(&mut material.header),
            Self::Optical(optical_action) => optical_action.apply(&mut material.optical),
        }
    }
}
/// Event emitted when any property of the material is modified.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialChangeEvent {
    /// The specific modification action.
    pub action: MaterialChangeAction,
}

/// The main editor component for optical materials.
#[component]
pub fn MaterialEditor(
    /// Controls if modal dialog should be displayed
    open: Signal<bool>,
    /// Read-only signal containing the complete material data.
    material: ReadSignal<Material>,

    /// Event handler triggered when properties inside the material change.
    on_change: EventHandler<MaterialChangeEvent>,

    /// Optional event handler triggered when the user clicks the save/publish button.
    #[props(default)]
    on_save: Option<EventHandler<()>>,

    /// Base ID used for HTML element IDs to avoid DOM collisions.
    #[props(default = "materialEditor".to_string())]
    base_id: String,

    /// If true, disables all input fields and actions.
    #[props(default = false)]
    readonly: bool,
) -> Element {
    info!("🔄 Render: MaterialEditor");
    // Derive memoized read-signals for child components to optimize re-rendering
    let header_memo = use_memo(move || material.read().header.clone());
    let optical_memo = use_memo(move || material.read().optical.clone());

    let current_version = material.read().version();
    let is_draft = current_version == 0;

    let handle_header_change = use_callback(move |event: AssetHeaderChangeEvent| {
        on_change.call(MaterialChangeEvent {
            action: MaterialChangeAction::Header(event.action),
        });
    });
    let handle_optical_change = use_callback(move |event: OpticalPropertiesChangeEvent| {
        on_change.call(MaterialChangeEvent {
            action: MaterialChangeAction::Optical(event.action),
        });
    });

    rsx! {
      AlertDialog {
        open: open(),
        on_open_change: move |v| open.set(v),
        max_width: "50rem".to_string(),
        AlertDialogTitle { "Material Editor" }
        AlertDialogDescription {
          ScrollArea { height: "45em",
            AssetHeaderEditor {
              header: header_memo,
              readonly,
              on_change: handle_header_change,
            }
            OpticalPropertiesEditor {
              optical: optical_memo,
              base_id: format!("{}_optical", base_id),
              readonly,
              on_change: handle_optical_change,
            }
          }
        }
        AlertDialogActions {
          AlertDialogCancel { "Cancel" }
          AlertDialogAction { on_click: move |_| {}, "Ok" }
        }
      }
      div { class: "material-editor-container", id: "{base_id}",

        // Header Bar with status indicator
        div { class: "d-flex justify-content-between align-items-center mb-3 pb-2 border-bottom",
          div {
            h4 { class: "mb-0",
              "{material.read().name()}"
              if is_draft {
                span { class: "badge bg-warning text-dark ms-2", "Draft (v0)" }
              } else {
                span { class: "badge bg-success ms-2", "Published (v{current_version})" }
              }
            }
          }

          // Save / Publish button
          if let Some(save_handler) = on_save {
            if !readonly {
              button {
                class: "btn btn-primary d-flex align-items-center",
                r#type: "button",
                onclick: move |_| save_handler.call(()),
                if is_draft {
                  "Publish to Registry"
                } else {
                  "Publish New Version"
                }
              }
            }
          }
        }
      
      // // 1. General Metadata Section (AssetHeader)
      // AssetHeaderEditor {
      //   header: header_memo,
      //   readonly,
      //   on_change: handle_header_change,
      // }

      // 2. Optical Properties Section
      // OpticalPropertiesEditor {
      //   optical: optical_memo,
      //   base_id: format!("{}_optical", base_id),
      //   readonly,
      //   on_change: handle_optical_change,
      // }
      }
    }
}
