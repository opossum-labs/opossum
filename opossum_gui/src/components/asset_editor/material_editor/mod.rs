pub mod mechanical_properties_editor;
pub mod optical_properties_editor;
pub mod thermal_properties_editor;

use super::asset_header_editor::{
    AssetHeaderChangeAction, AssetHeaderChangeEvent, AssetHeaderEditor,
};
use crate::components::primitives::{
    alert_dialog::{
        AlertDialog, AlertDialogAction, AlertDialogActions, AlertDialogCancel,
        AlertDialogDescription, AlertDialogTitle,
    },
    scroll_area::ScrollArea,
};
use dioxus::prelude::*;
use mechanical_properties_editor::{
    MechanicalPropertiesChangeAction, MechanicalPropertiesChangeEvent, MechanicalPropertiesEditor,
};
use opossum_core::material::Material;
use optical_properties_editor::{
    OpticalPropertiesChangeAction, OpticalPropertiesChangeEvent, OpticalPropertiesEditor,
};
use thermal_properties_editor::{
    ThermalPropertiesChangeAction, ThermalPropertiesChangeEvent, ThermalPropertiesEditor,
};

/// Actions representing modifications to a Material.
#[derive(Debug, Clone, PartialEq)]
pub enum MaterialChangeAction {
    Header(AssetHeaderChangeAction),
    Optical(OpticalPropertiesChangeAction),
    Thermal(ThermalPropertiesChangeAction),
    Mechanical(MechanicalPropertiesChangeAction),
    SetVersion(u32),
}

impl MaterialChangeAction {
    /// Applies the change action directly to the given `Material`
    pub fn apply(self, material: &mut opossum_core::material::Material) {
        match self {
            Self::Header(header_action) => header_action.apply(&mut material.header),
            Self::Optical(optical_action) => optical_action.apply(&mut material.optical),
            Self::Thermal(thermal_action) => thermal_action.apply(&mut material.thermal),
            Self::Mechanical(mech_action) => mech_action.apply(&mut material.mechanical),
            Self::SetVersion(version) => material.header.version = version,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialChangeEvent {
    pub action: MaterialChangeAction,
}

/// Helper component to handle `Option<f64>` inputs. Translates empty strings to `None`.
#[component]
pub fn OptionalPropertyInput(
    id: String,
    label: String,
    value: Option<f64>,
    on_change: EventHandler<Option<f64>>,
    #[props(default = false)] readonly: bool,
) -> Element {
    // Show empty string if the property is None
    let val_str = value.map(|v| v.to_string()).unwrap_or_default();

    let on_save = move |s: String| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            on_change.call(None);
        } else if let Ok(parsed) = trimmed.parse::<f64>() {
            on_change.call(Some(parsed));
        }
        // Invalid input is gracefully ignored (reverts to previous valid value on next render)
    };

    rsx! {
        crate::components::node_editor::inputs::input_components::FlushableTextInput {
            id,
            label,
            value: val_str,
            on_save,
            readonly,
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
            r#type: "number",
            step: "any",
        }
    }
}

#[component]
pub fn MaterialEditor(
    open: Signal<bool>,
    material: ReadSignal<Material>,
    on_change: EventHandler<MaterialChangeEvent>,
    #[props(default)] on_save: Option<EventHandler<()>>,
    #[props(default)] save_label: Option<String>,
    #[props(default = "materialEditor".to_string())] base_id: String,
    #[props(default = false)] readonly: bool,
) -> Element {
    debug!("🔄 Render: MaterialEditor");
    let mut show_overwrite_warning = use_signal(|| false);

    let header_memo = use_memo(move || material.read().header.clone());
    let optical_memo = use_memo(move || material.read().optical.clone());
    let thermal_memo = use_memo(move || material.read().thermal.clone());
    let mechanical_memo = use_memo(move || material.read().mechanical.clone());

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
    let handle_thermal_change = use_callback(move |event: ThermalPropertiesChangeEvent| {
        on_change.call(MaterialChangeEvent {
            action: MaterialChangeAction::Thermal(event.action),
        });
    });
    let handle_mechanical_change = use_callback(move |event: MechanicalPropertiesChangeEvent| {
        on_change.call(MaterialChangeEvent {
            action: MaterialChangeAction::Mechanical(event.action),
        });
    });

    let save_label_for_click = save_label.clone();
    let handle_save_click = use_callback(move |_| {
        if let Some(save_handler) = on_save {
            if is_draft || save_label_for_click.is_some() {
                save_handler.call(());
            } else {
                show_overwrite_warning.set(true);
            }
        }
    });

    rsx! {
        AlertDialog {
            open: open(),
            on_open_change: move |v| open.set(v),
            max_width: "50rem".to_string(),
            AlertDialogTitle { "Material Editor" }
            AlertDialogDescription {
                div { class: "material-editor-container", id: "{base_id}",
                    div { class: "d-flex justify-content-between align-items-center mb-3 pb-2 border-bottom",
                        h4 { class: "mb-0",
                            "{material.read().name()}"
                            if is_draft {
                                if save_label.is_some() {
                                    span { class: "badge bg-secondary ms-2", "AdHoc Material" }
                                } else {
                                    span { class: "badge bg-secondary ms-2", "Draft (Auto-Version)" }
                                }
                            } else {
                                span { class: "badge bg-warning text-dark ms-2",
                                    "Target Version: v{current_version}"
                                }
                            }
                        }
                    }
                }
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
                    ThermalPropertiesEditor {
                        thermal: thermal_memo,
                        base_id: format!("{}_thermal", base_id),
                        readonly,
                        on_change: handle_thermal_change,
                    }
                    MechanicalPropertiesEditor {
                        mechanical: mechanical_memo,
                        base_id: format!("{}_mechanical", base_id),
                        readonly,
                        on_change: handle_mechanical_change,
                    }

                    details { class: "mt-4 p-3 border rounded bg-light",
                        summary {
                            class: "fw-bold text-secondary text-uppercase small",
                            style: "cursor: pointer;",
                            "Advanced Settings (Expert Only)"
                        }
                        div { class: "mt-3",
                            div { class: "alert alert-warning py-2 px-3 small mb-2",
                                "Warning: Manually altering the version number bypasses the append-only database rule."
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
                                    step: "1",
                                    disabled: readonly,
                                    value: "{current_version}",
                                    onkeydown: move |evt| {
                                        if let Key::Character(ref c) = evt.key()
                                            && ["-", "+", ".", ",", "e", "E"].contains(&c.as_str())
                                        {
                                            evt.prevent_default();
                                        }
                                    },
                                    oninput: move |evt| {
                                        let raw_val = evt.value();
                                        if raw_val.is_empty() {
                                            on_change
                                                .call(MaterialChangeEvent {
                                                    action: MaterialChangeAction::SetVersion(0),
                                                });
                                        } else if let Ok(version_val) = raw_val.parse::<u32>() {
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
            AlertDialogActions {
                AlertDialogCancel { "Cancel" }
                if on_save.is_some() && !readonly {
                    AlertDialogAction { on_click: handle_save_click,
                        if let Some(custom_label) = save_label.as_deref() {
                            "{custom_label}"
                        } else if is_draft {
                            "Publish New Version"
                        } else {
                            "Overwrite Version {current_version}"
                        }
                    }
                }
            }
        }
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
