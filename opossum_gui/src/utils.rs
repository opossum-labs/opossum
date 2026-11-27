use heck::ToSnakeCase;

/// How long a view waits for the edits to stop before asking for the model again.
///
/// Long enough that holding a spinner or dragging a value through a range costs one fetch rather
/// than dozens, short enough that a single deliberate edit still feels immediate.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(100);

/// Wait out [`SETTLE`], on whichever platform this is.
pub async fn debounce() {
    #[cfg(feature = "desktop")]
    tokio::time::sleep(SETTLE).await;
    #[cfg(not(feature = "desktop"))]
    gloo_timers::future::sleep(SETTLE).await;
}

/// Extension trait providing sentence case transformation.
pub trait ToSentenceCase {
    /// Converts a string into sentence case (e.g., `optical_node` -> `Optical node`).
    fn to_sentence_case(&self) -> String;
}

impl<T: AsRef<str>> ToSentenceCase for T {
    fn to_sentence_case(&self) -> String {
        // Split into snake_case words, then replace underscores with spaces
        let words = self.as_ref().to_snake_case().replace('_', " ");
        let mut chars = words.chars();

        chars.next().map_or_else(String::new, |first| {
            // Capitalize the first character and append the remaining string
            let mut result = first.to_uppercase().to_string();
            result.push_str(chars.as_str());
            result
        })
    }
}
