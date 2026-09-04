use crate::APP_CONFIG;
use crate::components::menu_bar::project_helper::select_folder_path;
use crate::components::node_editor::inputs::input_components::{NodeConfigUnitInput, UnitHandling};
use crate::components::primitives::alert_dialog::{
    AlertDialog, AlertDialogAction, AlertDialogActions, AlertDialogCancel, AlertDialogDescription,
    AlertDialogTitle,
};
use dioxus::prelude::*;
use opossum_core::meter;

#[component]
pub fn SettingsDialog(open: Signal<bool>) -> Element {
    let mut active_tab = use_signal(|| 0);
    // Create a local temporary buffer of the entire configuration struct.
    let mut temp_config = use_signal(|| APP_CONFIG.read().clone());
    // Signal to store an active validation error message across dialog interactions.
    let mut wavelength_error = use_signal(|| None::<String>);

    use_effect(move || {
        if open() {
            *temp_config.write() = APP_CONFIG.read().clone();
            // Clear any lingering error state when opening the dialog.
            wavelength_error.set(None);
        }
    });

    rsx! {
        AlertDialog {
            open: open(),
            on_open_change: move |v: bool| open.set(v),
            max_width: "50rem".to_string(),
            AlertDialogTitle { "OPOSSUM Settings" }
            AlertDialogDescription {
                // Body with split layout
                div { class: "d-flex p-0", style: "min-height: 400px;",

                    // Left column: Navigation tabs
                    div { class: "list-group list-group-flush w-25 bg-dark border-end border-secondary",
                        button {
                            class: format!(
                                "list-group-item list-group-item-action py-3 border-0 {}",
                                if active_tab() == 0 {
                                    "bg-secondary text-white fw-bold"
                                } else {
                                    "bg-dark text-white-50"
                                },
                            ),
                            onclick: move |_| active_tab.set(0),
                            "General"
                        }
                        button {
                            class: format!(
                                "list-group-item list-group-item-action py-3 border-0 {}",
                                if active_tab() == 1 {
                                    "bg-secondary text-white fw-bold"
                                } else {
                                    "bg-dark text-white-50"
                                },
                            ),
                            onclick: move |_| active_tab.set(1),
                            "Physics / Model"
                        }
                    }

                    // Right column: Dynamic content
                    div {
                        class: "p-4 flex-grow-1",
                        style: "background-color: #1e1e1e; border-radius: 0 0 4px 0;",
                        match active_tab() {
                            0 => rsx! {
                                GeneralSettingsTab { temp_config }
                            },
                            1 => rsx! {
                                PhysicsSettingsTab { temp_config, wavelength_error }
                            },
                            _ => rsx! {
                                div { class: "text-danger", "Unknown category" }
                            },
                        }
                    }
                }
            }
            AlertDialogActions {
                AlertDialogCancel { "Cancel" }
                AlertDialogAction {
                    on_click: move |_| {
                        // Prevent persisting configuration if an active validation error exists.
                        if wavelength_error().is_some() {
                            eprintln!("Cannot save settings: validation error present");
                            return;
                        }

                        *APP_CONFIG.write() = temp_config.read().clone();
                        if let Err(e) = APP_CONFIG.read().to_file() {
                            eprintln!("Error while saving configuration: {e}");
                        }
                    },
                    "Save & Close"
                }
            }
        }
    }
}

#[component]
fn GeneralSettingsTab(mut temp_config: Signal<crate::AppConfig>) -> Element {
    let current_report_path_str = temp_config.read().report_dir().map_or_else(
        || "No default dir set".to_string(),
        |p| p.to_string_lossy().into_owned(),
    );

    let current_catalog_path_str = temp_config.read().catalog_dir().map_or_else(
        || "No default dir set".to_string(),
        |p| p.to_string_lossy().into_owned(),
    );

    let current_remote_url = temp_config.read().catalog_remote_url().to_string();
    let sync_on_startup = temp_config.read().sync_catalog_on_startup();

    rsx! {
        div { class: "d-flex flex-column gap-3",
            h4 { class: "mb-3", "General Settings" }

            // Report Base Directory Setting
            div { class: "form-group",
                label { class: "form-label text-muted small", "Report Base Directory" }
                div { class: "input-group",
                    input {
                        r#type: "text",
                        class: "form-control bg-dark text-white border-secondary",
                        style: "font-family: monospace; font-size: 13px;",
                        readonly: true,
                        value: "{current_report_path_str}",
                    }
                    button {
                        class: "btn btn-secondary",
                        r#type: "button",
                        onclick: move |_| {
                            let starting_dir = temp_config
                                .read()
                                .report_dir()
                                .filter(|p| p.exists())
                                .cloned();

                            spawn(async move {
                                if let Some(folder) = select_folder_path(
                                        starting_dir.as_deref(),
                                        Some("Select OPOSSUM report directory"),
                                    )
                                    .await
                                        && let Err(e) = temp_config.write().set_report_dir(&folder)
                                {
                                    eprintln!("Error setting report directory: {e}");
                                }
                            });
                        },
                        "Browse..."
                    }
                }
            }

            // Catalog Base Directory Setting
            div { class: "form-group",
                label { class: "form-label text-muted small",
                    "Catalog Directory (Materials, Coatings, etc.)"
                }
                div { class: "input-group",
                    input {
                        r#type: "text",
                        class: "form-control bg-dark text-white border-secondary",
                        style: "font-family: monospace; font-size: 13px;",
                        readonly: true,
                        value: "{current_catalog_path_str}",
                    }
                    button {
                        class: "btn btn-secondary",
                        r#type: "button",
                        onclick: move |_| {
                            let starting_dir = temp_config
                                .read()
                                .catalog_dir()
                                .filter(|p| p.exists())
                                .cloned();

                            spawn(async move {
                                if let Some(folder) = select_folder_path(
                                        starting_dir.as_deref(),
                                        Some("Select OPOSSUM catalog directory"),
                                    )
                                    .await
                                        && let Err(e) = temp_config.write().set_catalog_dir(&folder)
                                {
                                    eprintln!("Error setting catalog directory: {e}");
                                }
                            });
                        },
                        "Browse..."
                    }
                }
            }

            // Catalog Remote Git URL Setting
            div { class: "form-group",
                label { class: "form-label text-muted small", "Catalog Git Remote URL" }
                input {
                    r#type: "text",
                    class: "form-control bg-dark text-white border-secondary",
                    style: "font-family: monospace; font-size: 13px;",
                    value: "{current_remote_url}",
                    placeholder: "https://github.com/opossum-labs/opossum_catalog.git",
                    oninput: move |evt| {
                        temp_config.write().set_catalog_remote_url(evt.value());
                    },
                }
            }

            // Sync on Startup Toggle Switch
            div { class: "form-check form-switch pt-1",
                input {
                    r#type: "checkbox",
                    class: "form-check-input",
                    id: "syncCatalogOnStartupSwitch",
                    checked: sync_on_startup,
                    onchange: move |evt| {
                        temp_config.write().set_sync_catalog_on_startup(evt.checked());
                    },
                }
                label {
                    class: "form-check-label text-muted small user-select-none",
                    r#for: "syncCatalogOnStartupSwitch",
                    "Automatically check and update catalog repository on startup"
                }
            }
        }
    }
}

#[component]
fn PhysicsSettingsTab(
    mut temp_config: Signal<crate::AppConfig>,
    mut wavelength_error: Signal<Option<String>>,
) -> Element {
    // Tracks input revisions to force a dynamic remount when reverting an invalid value.
    let mut input_revision = use_signal(|| 0usize);
    let current_wavelength = temp_config.read().default_wavelength();

    rsx! {
        div { class: "d-flex flex-column gap-3",
            h4 { class: "mb-3", "Default Model Parameters" }

            div { class: "form-group",
                // Rendering inside a keyed iterator forces Dioxus to completely unmount and recreate
                // the input component and its underlying DOM node whenever `input_revision` changes.
                for rev in [input_revision()] {
                    div { key: "{rev}",
                        NodeConfigUnitInput {
                            id: "defaultWavelengthSetting",
                            label: "Default Wavelength".to_string(),
                            value: current_wavelength.value,
                            unit_config: UnitHandling::new("m", true),
                            readonly: false,
                            onchange: move |new_length: f64| {
                                match temp_config.write().set_default_wavelength(meter!(new_length)) {
                                    Ok(()) => {
                                        wavelength_error.set(None);
                                    }
                                    Err(err) => {
                                        wavelength_error.set(Some(err.to_string()));
                                    }
                                }
                            },
                        }
                    }
                }

                // Render visual validation warning if an error occurred
                if let Some(err_msg) = wavelength_error() {
                    div {
                        class: "alert alert-danger py-2 px-3 mt-2 mb-0 d-flex align-items-center justify-content-between small",
                        role: "alert",
                        div { class: "d-flex align-items-center gap-2",
                            span { class: "fw-semibold", "Invalid wavelength:" }
                            span { "{err_msg}" }
                        }
                        button {
                            class: "btn btn-sm btn-outline-light py-0 px-2",
                            r#type: "button",
                            onclick: move |_| {
                                // Clear the error banner and increment revision to force remount
                                wavelength_error.set(None);
                                input_revision.set(input_revision() + 1);
                            },
                            "Dismiss"
                        }
                    }
                }
            }
        }
    }
}
