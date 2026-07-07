use crate::APP_CONFIG;
use crate::components::menu_bar::project_helper::select_folder_path;
use dioxus::prelude::*;

#[component]
pub fn SettingsDialog(show: Signal<bool>) -> Element {
    // 1. CRITICAL: Hooks must always be at the very top of the component,
    // unconditionally, before any early returns!
    let mut active_tab = use_signal(|| 0);

    // Create a local temporary buffer of the entire configuration struct.
    let mut temp_config = use_signal(|| APP_CONFIG.read().clone());

    // 2. Synchronization: Whenever the dialog opens (show transitions to true),
    // we explicitly overwrite our local buffer with a fresh clone of the global configuration.
    use_effect(move || {
        if show() {
            *temp_config.write() = APP_CONFIG.read().clone();
        }
    });

    // Early return for visibility is now safely placed AFTER the hooks.
    if !show() {
        return rsx! {};
    }

    rsx! {
      div {
        class: "modal d-block",
        "tabindex": "-1",
        style: "background-color: rgba(0,0,0,0.5);",
        // Convenience feature: Close with Escape key
        onkeydown: move |evt| {
            if evt.key() == Key::Escape {
                show.set(false);
            }
        },
        // Automatically focus to capture keyboard inputs instantly
        onmounted: async move |evt| {
            let _ = evt.set_focus(true).await;
        },
        div { class: "modal-dialog modal-lg modal-dialog-centered",
          div { class: "modal-content bg-dark text-white",

            // Header (Clean without manual borders)
            div { class: "modal-header",
              h5 { class: "modal-title", "OPOSSUM Settings" }
              button {
                r#type: "button",
                class: "btn-close btn-close-white",
                onclick: move |_| show.set(false),
              }
            }

            // Body with split layout
            div {
              class: "modal-body d-flex p-0",
              style: "min-height: 400px;",

              // Left column: Navigation tabs with explicit active highlighting
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

              // Right column: Dynamic content embedded in consistent #1e1e1e console background
              div {
                class: "p-4 flex-grow-1",
                style: "background-color: #1e1e1e; border-radius: 0 0 4px 0;",
                match active_tab() {
                    0 => rsx! {
                      GeneralSettingsTab { temp_config }
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

            // Footer with separate buttons for Cancel and Save
            div { class: "modal-footer",
              button {
                r#type: "button",
                class: "btn btn-secondary",
                onclick: move |_| show.set(false),
                "Cancel"
              }
              button {
                r#type: "button",
                class: "btn btn-success", // Consistent green for positive actions
                onclick: move |_| {
                    // 3. Commit: Write the entire validated temporary struct back to the global state
                    *APP_CONFIG.write() = temp_config.read().clone();

                    // Save the updated configuration to disk
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
fn GeneralSettingsTab(mut temp_config: Signal<crate::AppConfig>) -> Element {
    // Read directly from the temporary configuration clone
    let current_path_str = temp_config.read().report_dir().map_or_else(
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
                          // Mutate only the temporary configuration clone
                          if let Err(e) = temp_config.write().set_report_dir(&folder) {
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
    rsx! {
      div { class: "d-flex flex-column gap-3",
        h4 { class: "mb-3", "Default Model Parameters" }
        p { class: "text-muted small", "Find future physics options here." }
      }
    }
}
