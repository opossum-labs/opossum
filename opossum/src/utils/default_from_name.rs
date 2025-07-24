use std::fmt::Display;
use strum::IntoEnumIterator;

pub trait DefaultFromName: IntoEnumIterator + Display + Clone + PartialEq {
    /// Creates a default instance of a type by name.
    ///
    /// This is used to instantiate a predefined type from a string input,
    /// e.g., in configuration files or UI selections.
    ///
    /// # Parameters
    /// - `name`: The name of the desired type.
    ///
    /// # Returns
    /// - `Some(type)` if the name is recognized.
    /// - `None` if the name is unknown.
    #[must_use]
    fn default_from_name(name: &str) -> Option<Self> {
        Self::iter().find(|ref_ind_type| format!("{ref_ind_type}") == name)
    }
}
