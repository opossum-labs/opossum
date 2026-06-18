#![allow(clippy::derive_partial_eq_without_eq)]

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
                    abbreviate_path(path, 40),
                    path.to_string_lossy().to_string(),
                )
            },
        )
    });
    let (display_path, full_path) = path_info();
    let modified_marker = if model_modified_sig() { "*" } else { "" };
    rsx! {
        li { class: "nav-item d-flex align-items-center",
            span { class: "navbar-text text-white-50 ms-3", title: "{full_path}",
                "{display_path} {modified_marker}"
            }
        }
    }
}

/// Abbreviates a path string to a maximum length, inserting '...' in the middle.
fn abbreviate_path(path: &Path, max_len: usize) -> String {
    let path_str = path.to_string_lossy();
    if path_str.len() <= max_len {
        return path_str.to_string();
    }

    let ellipsis = "...";
    // The length of the path string without the ellipsis
    let available_len = max_len.saturating_sub(ellipsis.len());
    // Allocate roughly half the available space to the prefix
    let prefix_len = available_len / 2;
    // The rest goes to the suffix
    let suffix_len = available_len - prefix_len;

    // Safely take the prefix using character iterators
    let prefix = path_str.chars().take(prefix_len).collect::<String>();

    // Safely take the suffix from the end of the string
    let suffix = path_str
        .chars()
        .skip(path_str.chars().count() - suffix_len)
        .collect::<String>();
    format!("{prefix}{ellipsis}{suffix}")
}
