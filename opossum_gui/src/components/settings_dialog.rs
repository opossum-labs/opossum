use crate::APP_CONFIG;
use crate::components::menu_bar::project_helper::select_folder_path;
use dioxus::prelude::*;
use std::path::PathBuf;

#[component]
pub fn SettingsDialog(
    show: Signal<bool>,
    mut project_directory: Signal<Option<PathBuf>>,
) -> Element {
    if !show() {
        return rsx! {};
    }

    let mut active_tab = use_signal(|| 0);

    rsx! {
      div {
        class: "modal d-block",
        "tabindex": "-1",
        style: "background-color: rgba(0,0,0,0.5);",
        // Komfort-Feature analog zum Simulationsfenster: Schließen mit Esc
        onkeydown: move |evt| {
            if evt.key() == Key::Escape {
                show.set(false);
            }
        },
        // Automatisch fokussieren, damit Tastatureingaben (wie Esc) sofort funktionieren
        onmounted: async move |evt| {
            let _ = evt.set_focus(true).await;
        },
        div { class: "modal-dialog modal-lg modal-dialog-centered",
          div { class: "modal-content bg-dark text-white",

            // Header (Clean ohne manuelle Rahmenstriche)
            div { class: "modal-header",
              h5 { class: "modal-title", "OPOSSUM Settings" }
              button {
                r#type: "button",
                class: "btn-close btn-close-white",
                onclick: move |_| show.set(false),
              }
            }

            // Body mit Split-Layout
            div {
              class: "modal-body d-flex p-0",
              style: "min-height: 400px;", // Korrigiert von '400 char' zu '400px'

              // Linke Spalte: Navigations-Tabs im einheitlichen Dark-Look
              div { class: "list-group list-group-flush w-25 bg-dark border-end border-secondary",
                button {
                  class: format!(
                      "list-group-item list-group-item-action text-white bg-dark py-3 border-0 {}",
                      if active_tab() == 0 { "active bg-secondary" } else { "" },
                  ),
                  onclick: move |_| active_tab.set(0),
                  "General"
                }
                button {
                  class: format!(
                      "list-group-item list-group-item-action text-white bg-dark py-3 border-0 {}",
                      if active_tab() == 1 { "active bg-secondary" } else { "" },
                  ),
                  onclick: move |_| active_tab.set(1),
                  "Physics / Model"
                }
              }

              // Rechte Spalte: Dynamischer Inhalt eingebettet im konsistenten #1e1e1e Konsolen-Hintergrund
              div {
                class: "p-4 flex-grow-1",
                style: "background-color: #1e1e1e; border-radius: 0 0 4px 0;",
                match active_tab() {
                    0 => rsx! {
                      GeneralSettingsTab { project_directory }
                    },
                    1 => rsx! {
                      PhysicsSettingsTab {}
                    },
                    _ => rsx! {
                      div { class: "text-danger", "Unknown category" }
                    },
                }
              }
            }

            // Footer mit getrennten Buttons für "Abbrechen" und "Speichern"
            div { class: "modal-footer",
              button {
                r#type: "button",
                class: "btn btn-secondary",
                onclick: move |_| show.set(false),
                "Cancel"
              }
              button {
                r#type: "button",
                class: "btn btn-success", // Konsistentes Grün für positive Aktionen
                onclick: move |_| {
                    if let Err(e) = APP_CONFIG.read().to_file() {
                        eprintln!("Error while saving configuration: {e}");
                    }
                    show.set(false);
                },
                "Save & Close"
              }
            }
          }
        }
      }
    }
}

#[component]
fn GeneralSettingsTab(mut project_directory: Signal<Option<PathBuf>>) -> Element {
    let config = APP_CONFIG();
    let current_path_str = config.report_dir().map_or_else(
        || "No default dir set".to_string(),
        |p| p.to_string_lossy().into_owned(),
    );

    rsx! {
      div { class: "d-flex flex-column gap-3",
        h4 { class: "mb-3", "General Settings" }

        div { class: "form-group",
          label { class: "form-label text-muted small", "Report Base Directory" }
          div { class: "input-group",
            input {
              r#type: "text",
              class: "form-control bg-dark text-white border-secondary",
              style: "font-family: monospace; font-size: 13px;",
              readonly: true,
              value: "{current_path_str}",
            }
            button {
              class: "btn btn-secondary",
              r#type: "button",
              onclick: move |_| {
                  spawn(async move {
                      if let Some(folder) = select_folder_path().await {
                          project_directory.set(Some(folder.clone()));
                          let mut app_config = APP_CONFIG.write();
                          if let Err(e) = app_config.set_report_dir(&folder) {
                              eprintln!("Error setting directory: {e}");
                          }
                      }
                  });
              },
              "Browse..."
            }
          }
        }
      }
    }
}

#[component]
fn PhysicsSettingsTab() -> Element {
    // let config = APP_CONFIG();

    rsx! {
      div { class: "d-flex flex-column gap-3",
        h4 { class: "mb-3", "Default Model Parameters" }
        p { class: "text-muted small", "Find future physics options here." }
      }
    }
}
