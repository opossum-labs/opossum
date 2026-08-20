pub mod optical_properties_editor;

use crate::components::primitives::{
    alert_dialog::{
        AlertDialog, AlertDialogAction, AlertDialogActions, AlertDialogCancel,
        AlertDialogDescription, AlertDialogTitle,
    },
    scroll_area::ScrollArea,
};
use dioxus::prelude::*;
use opossum_core::material::Material;

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
    /// Explicitly sets the version number (0 = draft for next auto-version, >0 = target specific version).
    SetVersion(u32),
}

impl MaterialChangeAction {
    /// Applies the change action directly to the given `Material`
    /// by delegating to the specific sub-actions.
    pub fn apply(self, material: &mut opossum_core::material::Material) {
        match self {
            Self::Header(header_action) => header_action.apply(&mut material.header),
            Self::Optical(optical_action) => optical_action.apply(&mut material.optical),
            Self::SetVersion(version) => material.header.version = version,
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
    /// Controls if the main modal dialog should be displayed.
    open: Signal<bool>,
    /// Read-only signal containing the complete material data.
    material: ReadSignal<Material>,

    /// Event handler triggered when properties inside the material change.
    on_change: EventHandler<MaterialChangeEvent>,

    /// Optional event handler triggered when the user saves/publishes the asset.
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

    // Local state to control the overwrite confirmation dialog
    let mut show_overwrite_warning = use_signal(|| false);

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

    // Handler that checks whether a direct save or a confirmation warning is required
    let handle_save_click = use_callback(move |_| {
        if let Some(save_handler) = on_save {
            if is_draft {
                // Drafts safely publish as next version (latest + 1)
                save_handler.call(());
            } else {
                // Existing version: Require explicit user confirmation before overwriting
                show_overwrite_warning.set(true);
            }
        }
    });

    rsx! {
      // 1. Main Material Editor Dialog
      AlertDialog {
        open: open(),
        on_open_change: move |v| open.set(v),
        max_width: "50rem".to_string(),
        AlertDialogTitle { "Material Editor" }
        AlertDialogDescription {
          div { class: "material-editor-container", id: "{base_id}",

            // Clean Header Bar: Displays asset title and status badge
            div { class: "d-flex justify-content-between align-items-center mb-3 pb-2 border-bottom",
              h4 { class: "mb-0",
                "{material.read().name()}"
                if is_draft {
                  span { class: "badge bg-secondary ms-2", "Draft (Auto-Version)" }
                } else {
                  span { class: "badge bg-warning text-dark ms-2",
                    "Target Version: v{current_version}"
                  }
                }
              }
            }
          }

          // Main Scroll Area for Material Attributes
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

            // Advanced / Dangerous Options: Positioned at the very bottom
            details { class: "mt-4 p-3 border rounded bg-light",
              summary {
                class: "fw-bold text-secondary text-uppercase small",
                style: "cursor: pointer;",
                "Advanced Settings (Expert Only)"
              }
              div { class: "mt-3",
                div { class: "alert alert-warning py-2 px-3 small mb-2",
                  "Warning: Manually altering the version number bypasses the append-only database rule. Overwriting existing versions may cause merge conflicts when synchronizing with remote repositories."
                }
                div { class: "d-flex align-items-center gap-2",
                  label {
                    class: "form-label mb-0 small text-muted",
                    r#for: "{base_id}_version_input",
                    "Target Version Number:"
                  }
                  input {
                    id: "{base_id}_version_input",
                    class: "form-control form-control-sm text-center",
                    style: "width: 5.5rem;",
                    r#type: "number",
                    min: "0",
                    disabled: readonly,
                    value: "{current_version}",
                    oninput: move |evt| {
                        if let Ok(version_val) = evt.value().parse::<u32>() {
                            on_change
                                .call(MaterialChangeEvent {
                                    action: MaterialChangeAction::SetVersion(version_val),
                                });
                        }
                    },
                  }
                  span { class: "small text-muted",
                    if is_draft {
                      "(0 = Assign next available version automatically)"
                    } else {
                      "(Will overwrite version {current_version} on disk)"
                    }
                  }
                }
              }
            }
          }
        }

        // Primary Action Footer
        AlertDialogActions {
          AlertDialogCancel { "Cancel" }

          if on_save.is_some() && !readonly {
            AlertDialogAction { on_click: handle_save_click,
              if is_draft {
                "Publish New Version"
              } else {
                "Overwrite Version {current_version}"
              }
            }
          }
        }
      }

      // 2. Overwrite Confirmation Warning Dialog
      AlertDialog {
        open: show_overwrite_warning(),
        on_open_change: move |v| show_overwrite_warning.set(v),
        max_width: "35rem".to_string(),
        AlertDialogTitle { "Confirm Version Overwrite" }
        AlertDialogDescription {
          div { class: "text-danger fw-bold mb-2",
            "Attention: You are about to overwrite version {current_version}!"
          }
          p { class: "small text-muted mb-0",
            "This operation replaces the existing version file on disk. If this version has already been pushed to a remote repository, this change can cause Git merge conflicts during synchronization."
          }
        }
        AlertDialogActions {
          AlertDialogCancel { "Cancel" }
          AlertDialogAction {
            on_click: move |_| {
                if let Some(save_handler) = on_save {
                    save_handler.call(());
                }
            },
            "Yes, Overwrite Version"
          }
        }
      }
    }
}
