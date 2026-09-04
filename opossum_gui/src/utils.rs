use heck::ToSnakeCase;

/// Extension trait providing sentence case transformation.
pub trait ToSentenceCase {
    /// Converts a string into sentence case (e.g., "optical_node" -> "Optical node").
    fn to_sentence_case(&self) -> String;
}

impl<T: AsRef<str>> ToSentenceCase for T {
    fn to_sentence_case(&self) -> String {
        // Split into snake_case words, then replace underscores with spaces
        let words = self.as_ref().to_snake_case().replace('_', " ");
        let mut chars = words.chars();

        match chars.next() {
            None => String::new(),
            Some(first) => {
                // Capitalize the first character and append the remaining string
                let mut result = first.to_uppercase().to_string();
                result.push_str(chars.as_str());
                result
            }
        }
    }
}
