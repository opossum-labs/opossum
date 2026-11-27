// src/components/menu_bar/file_path_display/mod.rs
use dioxus::prelude::*;
use std::path::{Path, PathBuf};

#[component]
pub fn FilePathDisplay(
    model_file_path: ReadSignal<Option<PathBuf>>,
    model_modified_sig: ReadSignal<bool>,
) -> Element {
    let path_info = use_memo(move || {
        model_file_path().as_ref().map_or_else(
            || {
                (
                    "unsaved.opm".to_string(),
                    "this model has not been saved yet".to_string(),
                )
            },
            |path| {
                (
                    abbreviate_path(path, 80),
                    path.to_string_lossy().to_string(),
                )
            },
        )
    });
    let (display_path, full_path) = path_info();
    let modified_marker = if model_modified_sig() { "*" } else { "" };

    rsx! {
        div {
            class: "d-flex align-items-center ms-3",
            style: "flex: 0 1 auto; min-width: 0; overflow: hidden;",
            span {
                class: "navbar-text text-white-50 text-truncate d-block",
                title: "{full_path}",
                "{display_path}{modified_marker}"
            }
        }
    }
}

/// Abbreviates a path string to a maximum character length, inserting '...' in the middle.
fn abbreviate_path(path: &Path, max_len: usize) -> String {
    let path_str = path.to_string_lossy();
    let total_chars = path_str.chars().count();
    if total_chars <= max_len {
        return path_str.to_string();
    }

    let ellipsis = "...";
    let available_len = max_len.saturating_sub(ellipsis.len());
    let prefix_len = available_len / 2;
    let suffix_len = available_len - prefix_len;

    let prefix = path_str.chars().take(prefix_len).collect::<String>();
    let suffix = path_str
        .chars()
        .skip(total_chars.saturating_sub(suffix_len))
        .collect::<String>();

    format!("{prefix}{ellipsis}{suffix}")
}
