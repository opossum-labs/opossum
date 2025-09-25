use std::path::Path;

/// Abbreviates a path string to a maximum length, inserting '...' in the middle.
pub fn abbreviate_path(path: &Path, max_len: usize) -> String {
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
        format!("{}{}{}", prefix, ellipsis, suffix)
}
