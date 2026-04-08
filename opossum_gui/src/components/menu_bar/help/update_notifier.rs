use dioxus::prelude::*;
use opossum_core::types::api_types::VersionInfo; // Passe den Pfad an

#[component]
pub fn UpdateNotifier(version_info: VersionInfo) -> Element {
    // Zeige nur etwas an, wenn ein Update verfügbar ist und wir eine URL haben
    if version_info.update_available
        && let (Some(latest_version), Some(url)) =
            (version_info.latest_github_version, version_info.release_url)
    {
        return rsx! {
          div { class: "alert alert-info mt-3", role: "alert",
            strong { "New update available! " }
            br {}
            "Version "
            {latest_version}
            " available on GitHub."
            br {}
            a {
              href: "{url}",
              target: "_blank",
              rel: "noopener noreferrer",
              class: "btn btn-sm btn-primary mt-2",
              "Show Release"
            }
          }
        };
    }
    // Do not render anything if no new version or offline
    rsx! {
      div { display: "none" }
    }
}
