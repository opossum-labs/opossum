use crate::{
    APP_CONFIG,
    components::primitives::button::{Button, ButtonSize, ButtonVariant},
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaCheck, FaCircleExclamation, FaCloudArrowDown, FaCodeBranch},
};

/// Props for the Git synchronization banner component.
#[derive(Props, Clone, PartialEq)]
pub struct GitSyncBannerProps {
    /// Indicates whether a Git pull/sync operation is currently in progress.
    pub is_syncing: bool,
    /// Holds the result of the last sync operation, if available.
    pub sync_result: Option<Result<String, String>>,
    /// Triggered when the user clicks the pull updates button.
    pub on_sync: EventHandler<()>,
}

/// Renders a banner displaying the shared catalog Git repository details and sync controls.
#[component]
pub fn GitSyncBanner(props: GitSyncBannerProps) -> Element {
    let config = APP_CONFIG.read();
    let remote_url = config.catalog_remote_url().to_string();
    let catalog_dir = config.catalog_dir().cloned();

    rsx! {
      div { class: "p-3 mb-4 rounded border bg-light text-dark",
        // Header row with repository details and sync button
        div { class: "d-flex justify-content-between align-items-center flex-wrap gap-3",
          div {
            div { class: "fw-bold small d-flex align-items-center gap-1",
              Icon { icon: FaCodeBranch }
              "Shared Git Catalog Repository"
            }
            div { class: "text-muted small", "{remote_url}" }
            if let Some(dir) = catalog_dir {
              div { class: "text-muted small font-monospace", "{dir.display()}" }
            }
          }
          div {
            Button {
              variant: ButtonVariant::Primary,
              size: ButtonSize::Sm,
              disabled: props.is_syncing,
              onclick: move |_| props.on_sync.call(()),
              Icon { icon: FaCloudArrowDown }
              if props.is_syncing {
                "Syncing with Git..."
              } else {
                "Pull Updates from Git"
              }
            }
          }
        }

        // Status feedback notification
        if let Some(ref res) = props.sync_result {
          match res {
              Ok(msg) => rsx! {
                div { class: "alert alert-success mt-2 mb-0 py-1 px-2 small d-flex align-items-center gap-2",
                  Icon { icon: FaCheck }
                  "{msg}"
                }
              },
              Err(err) => rsx! {
                div { class: "alert alert-danger mt-2 mb-0 py-1 px-2 small d-flex align-items-center gap-2",
                  Icon { icon: FaCircleExclamation }
                  "{err}"
                }
              },
          }
        }
      }
    }
}
