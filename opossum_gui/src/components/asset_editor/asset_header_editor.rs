use dioxus::prelude::*;
use opossum_core::asset::AssetHeader;

use crate::components::node_editor::inputs::input_components::LabeledInput;

/// Defines the specific field that was modified in the asset header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetHeaderChangeAction {
    /// The name of the asset changed.
    Name(String),
    /// The manufacturer changed (None if the field was cleared).
    Manufacturer(Option<String>),
    /// The description changed (None if the field was cleared).
    Description(Option<String>),
}
impl AssetHeaderChangeAction {
    /// Applies the change action directly to the given `AssetHeader`.
    pub fn apply(self, header: &mut opossum_core::asset::AssetHeader) {
        match self {
            Self::Name(name) => header.name = name,
            Self::Manufacturer(mfg) => header.manufacturer = mfg,
            Self::Description(desc) => header.description = desc,
        }
    }
}
/// Event emitted when the user changes a value in the `AssetHeaderEditor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetHeaderChangeEvent {
    /// The specific action / field modification.
    pub action: AssetHeaderChangeAction,
}

/// Properties for the `AssetHeaderEditor` component.
#[derive(Props, Clone, PartialEq)]
pub struct AssetHeaderEditorProps {
    /// Read-only signal containing the current state of the asset header.
    pub header: ReadSignal<AssetHeader>,
    /// Event handler triggered when any input field changes.
    pub on_change: EventHandler<AssetHeaderChangeEvent>,
    /// If true, disables all input fields.
    #[props(default = false)]
    pub readonly: bool,
}

/// Reusable editor component for the common `AssetHeader` fields.
///
/// Follows the "props down, events up" pattern. Changes are emitted via `on_change`.
#[component]
pub fn AssetHeaderEditor(props: AssetHeaderEditorProps) -> Element {
    info!("🔄 Render: AssetHeaderEditor");
    let header = props.header.read();
    rsx! {
      div { class: "card mb-4",
        div { class: "card-header bg-light",
          h5 { class: "mb-0", "General Asset Information" }
        }
        div { class: "card-body",

          // Row 1: Name and Manufacturer
          div { class: "row mb-3",
            div { class: "col-md-6",
              LabeledInput {
                id: "name",
                label: "Name*",
                value: header.name.clone(),
                onchange: move |e: Event<FormData>| {
                    props
                        .on_change
                        .call(AssetHeaderChangeEvent {
                            action: AssetHeaderChangeAction::Name(e.value()),
                        });
                },
              }
            }

            div { class: "col-md-6",

              LabeledInput {
                id: "manufacturer",
                label: "Manufacturer",
                value: header.manufacturer.clone().unwrap_or_default(),
                onchange: move |e: Event<FormData>| {
                    let val = e.value();
                    let opt_val = if val.trim().is_empty() {
                        None
                    } else {
                        Some(val.trim().to_string())
                    };
                    props
                        .on_change
                        .call(AssetHeaderChangeEvent {
                            action: AssetHeaderChangeAction::Manufacturer(opt_val),
                        });
                },
              }
            }
          }

          // Row 2: Description
          div { class: "row mb-3",
            div { class: "col-12",
              LabeledInput {
                id: "description",
                label: "Description",
                value: header.description.clone().unwrap_or_default(),
                onchange: move |e: Event<FormData>| {
                    let val = e.value();
                    let opt_val = if val.trim().is_empty() {
                        None
                    } else {
                        Some(val.trim().to_string())
                    };
                    props
                        .on_change
                        .call(AssetHeaderChangeEvent {
                            action: AssetHeaderChangeAction::Description(opt_val),
                        });
                },
              }
            }
          }

          hr {}

          // Row 3: Read-only system metadata (UUID, Schema, Version)
          div { class: "row text-muted small",
            div { class: "col-md-5",
              span { class: "fw-bold me-1", "Asset ID:" }
              span { class: "user-select-all", "{header.id}" }
            }
            div { class: "col-md-4",
              span { class: "fw-bold me-1", "Version:" }
              span {
                // Version 0 indicates a local draft
                if header.version == 0 {
                  "Unpublished Draft (v0)"
                } else {
                  "v{header.version}"
                }
              }
            }
            div { class: "col-md-3",
              span { class: "fw-bold me-1", "Schema:" }
              span { "v{header.schema_version}" }
            }
          }
        }
      }
    }
}
