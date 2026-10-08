//! Upgrading `.opm` files written by older versions of OPOSSUM.
//!
//! Every file states the version of its format (`opm_file_version`). A file of an older version
//! is brought up to the current one when it is read and stamped with the current version, so it
//! is written back in the current format. A file of a newer version, or one whose version cannot
//! be read, is read as it is, with a warning.

use super::OpmDocument;
use crate::error::{OpmResult, OpossumError};
use log::{info, warn};

/// Return the version of the file format this program writes.
///
/// # Errors
///
/// This function returns an error if the version this program was built with is not a number.
fn current_version() -> OpmResult<u32> {
    env!("OPM_FILE_VERSION").parse().map_err(|e| {
        OpossumError::OpmDocument(format!(
            "the file format version '{}' of this program is not a number: {e}",
            env!("OPM_FILE_VERSION")
        ))
    })
}

/// Bring a document read from a file up to the file format this program writes.
///
/// # Arguments
///
/// * `document` - the document as read from the file.
///
/// # Errors
///
/// This function returns an error if the version this program writes is not a number.
pub(super) fn upgrade(document: &mut OpmDocument) -> OpmResult<()> {
    let current = current_version()?;
    match document.opm_file_version.parse::<u32>() {
        Ok(read) if read == current => {}
        Ok(read) if read < current => {
            info!("upgrading the model from file format version {read} to {current}");
            document.opm_file_version = current.to_string();
        }
        _ => {
            warn!("OPM file version does not match the used OPOSSUM version.");
            warn!(
                "read version '{}' <-> program file version '{current}'",
                document.opm_file_version,
            );
            warn!(
                "This file might have been written by an older or newer version of OPOSSUM. The model import might not be correct."
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{error::OpmResult, opm_document::OpmDocument};

    /// A document of the current version, serialized and then stated to be of `version`.
    fn file_of_version(version: &str) -> OpmResult<String> {
        let current = format!("opm_file_version: \"{}\"", env!("OPM_FILE_VERSION"));
        let file = OpmDocument::default().to_opm_file_string()?;
        assert!(file.contains(&current), "the file states its version");
        Ok(file.replace(&current, &format!("opm_file_version: \"{version}\"")))
    }
    /// Read a file and return the version of the document and whether a warning was logged.
    fn read(file: &str) -> OpmResult<(String, bool)> {
        testing_logger::setup();
        let document = OpmDocument::from_string(file)?;
        let warned = std::cell::Cell::new(false);
        testing_logger::validate(|logs| {
            warned.set(logs.iter().any(|log| log.level == log::Level::Warn));
        });
        Ok((document.opm_file_version, warned.get()))
    }
    #[test]
    fn a_file_of_an_older_version_is_upgraded_to_the_current_one() -> OpmResult<()> {
        let (version, warned) = read(&file_of_version("0")?)?;
        assert_eq!(version, env!("OPM_FILE_VERSION"));
        assert!(!warned, "an upgrade is expected, not a reason to warn");
        Ok(())
    }
    #[test]
    fn a_file_of_the_current_version_is_read_as_it_is() -> OpmResult<()> {
        let (version, warned) = read(&file_of_version(env!("OPM_FILE_VERSION"))?)?;
        assert_eq!(version, env!("OPM_FILE_VERSION"));
        assert!(!warned);
        Ok(())
    }
    /// A newer file may hold what this program does not know; it is read with a warning and keeps
    /// its version. So does a file whose version is not a number.
    #[test]
    fn a_file_of_a_newer_or_unreadable_version_is_read_with_a_warning() -> OpmResult<()> {
        for version in ["1000", "abc"] {
            let (read_version, warned) = read(&file_of_version(version)?)?;
            assert_eq!(read_version, version);
            assert!(warned, "version {version}");
        }
        Ok(())
    }
}
