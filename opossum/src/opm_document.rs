use crate::{
    analyzers::AnalyzerType,
    error::{OpmResult, OpossumError},
    OpticScenery,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct OpmDocument {
    opm_file_version: String,
    #[serde(default)]
    scenery: OpticScenery,
    #[serde(default)]
    analyzers: Vec<AnalyzerType>,
}
impl Default for OpmDocument {
    fn default() -> Self {
        Self {
            opm_file_version: env!("OPM_FILE_VERSION").to_string(),
            scenery: OpticScenery::default(),
            analyzers: vec![],
        }
    }
}
impl OpmDocument {
    /// Creates a new [`OpmDocument`].
    #[must_use]
    pub fn new(scenery: OpticScenery) -> Self {
        Self {
            scenery,
            ..Default::default()
        }
    }
    /// Create a new [`OpmDocument`] from an `.opm` file at the given path.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the given path is not found or readable.
    ///   - the parsing / deserialization of the file failed.
    pub fn from_file(path: &Path) -> OpmResult<Self> {
        let contents = fs::read_to_string(path).map_err(|e| {
            OpossumError::OpmDocument(format!("cannot read file {} : {}", path.display(), e))
        })?;
        let document: Self = serde_yaml::from_str(&contents)
            .map_err(|e| OpossumError::OpmDocument(format!("parsing of model failed: {e}")))?;
        Ok(document)
    }
    /// Save this [`OpmDocument`] to an `.opm` file with the given path
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the serialization of the document failed.
    ///   - the file path cannot be created.
    ///   - it cannot write into the file (e.g. no space).
    pub fn save_to_file(&self, path: &Path) -> OpmResult<()> {
        let serialized = serde_yaml::to_string(&self).map_err(|e| {
            OpossumError::OpticScenery(format!("deserialization of OpmDocument failed: {e}"))
        })?;
        let mut output = File::create(path).map_err(|e| {
            OpossumError::OpticScenery(format!(
                "could not create file path: {}: {}",
                path.display(),
                e
            ))
        })?;
        write!(output, "{serialized}").map_err(|e| {
            OpossumError::OpticScenery(format!(
                "writing to file path {} failed: {}",
                path.display(),
                e
            ))
        })?;
        Ok(())
    }
}

// todo: Check file version during deserialization
