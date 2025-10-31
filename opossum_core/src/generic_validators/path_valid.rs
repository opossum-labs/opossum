use std::path::PathBuf;

use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::Validate,
};
use serde::{Deserialize, Serialize};

/// Validator that checks if a path is valid
///
/// This includes if the extension is correct and if the path exists
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct PathValid {
    ext: Option<Vec<String>>,
}
impl Default for PathValid {
    fn default() -> Self {
        panic!(
            "PathValid::default() is a dummy implementation to facilitate using serde(skip) on Validator fields in the Validated struct!\nAlways implement Deserialize a manually for every struct that holds a validated type with an PathValid Validator to ensure that all parameters are set correctly!"
        );
    }
}
impl PathValid {
    /// Create a new `PathValid` validator.
    ///
    /// # Arguments
    ///
    /// * `ext` - allowed extensions of the path. Accepts everything when set to None, to "*" or when empty. Dots and whitespaces are trimmed from the passed string.
    #[must_use]
    pub fn new(ext_opt: Option<Vec<&'static str>>) -> Self {
        let ext = ext_opt.and_then(|ext_vec| {
            if ext_vec.is_empty() {
                None
            } else {
                let mut ext_vec_trimmed = Vec::<String>::new();
                for ext in ext_vec {
                    let no_dots = ext.replace('.', "");
                    let trimmed = no_dots.trim();
                    if trimmed.is_empty() || trimmed == "*" {
                        continue;
                    }
                    ext_vec_trimmed.push(trimmed.to_string());
                }
                if ext_vec_trimmed.is_empty() {
                    None
                } else {
                    Some(ext_vec_trimmed)
                }
            }
        });

        Self { ext }
    }
}

impl Validate<PathBuf> for PathValid {
    fn validate(&self, path_buf: &PathBuf) -> OpmResult<()> {
        let path = path_buf.as_path();

        self.ext.as_ref().map_or(Ok(()), |ext_vec| {
            if let Some(Some(extension)) = path.extension().map(|s| s.to_str()) {
                if ext_vec.contains(&extension.to_string()) {
                    Ok(())
                } else {
                    Err(OpossumError::Other(format!(
                        "Path extension must be \"{ext_vec:?}\""
                    )))
                }
            } else {
                Err(OpossumError::Other(
                    "Path extension cannot be extracted!".to_string(),
                ))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn logo_path() -> PathBuf {
        PathBuf::from("./logo/Logo_square_tiny_grey_inverted.png")
    }

    #[test]
    fn accepts_any_extension_when_none() {
        let validator = PathValid::new(None);
        assert!(validator.validate(&logo_path()).is_ok());
    }

    #[test]
    fn accepts_any_extension_when_star() {
        let validator = PathValid::new(Some(vec!["*"]));
        assert!(validator.validate(&logo_path()).is_ok());
    }

    #[test]
    fn accepts_any_extension_when_empty_vec() {
        let validator = PathValid::new(Some(vec![]));
        assert!(validator.validate(&logo_path()).is_ok());
    }

    #[test]
    fn accepts_correct_extension() {
        let validator = PathValid::new(Some(vec!["png"]));
        assert!(validator.validate(&logo_path()).is_ok());
    }

    #[test]
    fn accepts_correct_extension_with_dot_and_whitespace() {
        let validator = PathValid::new(Some(vec![" .png "]));
        assert!(validator.validate(&logo_path()).is_ok());
    }

    #[test]
    fn rejects_incorrect_extension() {
        let validator = PathValid::new(Some(vec!["jpg", "jpeg"]));
        let result = validator.validate(&logo_path());
        assert!(
            result.is_err(),
            "Expected validation error for invalid extension"
        );
        if let Err(OpossumError::Other(msg)) = result {
            assert!(
                msg.contains("Path extension must be"),
                "Unexpected error message: {msg}"
            );
        } else {
            panic!("Unexpected error type");
        }
    }

    #[test]
    fn rejects_file_with_no_extension() {
        let validator = PathValid::new(Some(vec!["png"]));
        let path = PathBuf::from("file_without_extension");
        let result = validator.validate(&path);
        assert!(result.is_err(), "Expected error when file has no extension");
        if let Err(OpossumError::Other(msg)) = result {
            assert!(
                msg.contains("cannot be extracted"),
                "Unexpected error message: {msg}"
            );
        }
    }

    #[test]
    fn trims_dot_and_whitespace_from_extensions() {
        let validator = PathValid::new(Some(vec![" .png ", " .jpg "]));
        assert_eq!(
            validator.ext,
            Some(vec!["png".to_string(), "jpg".to_string()])
        );
    }

    #[test]
    fn filters_out_invalid_or_wildcard_extensions() {
        let validator = PathValid::new(Some(vec!["*", ".", "  ", ".png", "txt"]));
        assert_eq!(
            validator.ext,
            Some(vec!["png".to_string(), "txt".to_string()])
        );
    }

    #[test]
    fn returns_none_when_all_invalid_extensions_removed() {
        let validator = PathValid::new(Some(vec!["*", ".", " "]));
        assert_eq!(validator.ext, None);
    }
}
