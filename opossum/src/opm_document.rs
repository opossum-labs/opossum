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
    #[serde(rename = "opm file version")]
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
    pub fn add_analyzer(&mut self, analyzer: AnalyzerType) {
        self.analyzers.push(analyzer);
    }
    pub fn scenery_mut(&mut self) -> &mut OpticScenery {
        &mut self.scenery
    }
    pub fn analyzers(&self) -> &[AnalyzerType] {
        &self.analyzers
    }
}

#[cfg(test)]
mod test {
    use crate::analyzers::RayTraceConfig;

    use super::*;
    use petgraph::adj::NodeIndex;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    #[test]
    fn new() {
        let mut scenery = OpticScenery::default();
        scenery.set_description("MyTest");
        let document = OpmDocument::new(scenery);
        assert_eq!(document.scenery.description(), "MyTest");
        assert!(document.analyzers.is_empty());
    }
    #[test]
    fn default() {
        let document = OpmDocument::default();
        assert_eq!(document.opm_file_version, env!("OPM_FILE_VERSION"));
        assert!(document.analyzers.is_empty());
    }
    #[test]
    fn from_file() {
        let result =
            OpmDocument::from_file(&Path::new("./invalid_file_path/invalid_file.invalid_ext"));
        assert!(result.unwrap_err().to_string().starts_with(
            "OpmDocument:cannot read file ./invalid_file_path/invalid_file.invalid_ext"
        ));
        let result =
            OpmDocument::from_file(&Path::new("./files_for_testing/opm/incorrect_opm.opm"));
        assert_eq!(
            result.unwrap_err().to_string(),
            "OpmDocument:parsing of model failed: missing field `opm file version`"
        );

        let document =
            OpmDocument::from_file(&PathBuf::from("./files_for_testing/opm/opticscenery.opm"))
                .unwrap();
        let node1 = document.scenery.node(NodeIndex::from(0)).unwrap();
        let node2 = document.scenery.node(NodeIndex::from(1)).unwrap();
        assert_eq!(
            "180328fe-7ad4-4568-b501-183b88c4daee",
            node1.uuid().to_string()
        );
        assert_eq!(
            "642ce76e-b071-43c0-a77e-1bdbb99b40d8",
            node2.uuid().to_string()
        );
    }
    #[test]
    fn save_to_file() {
        let file = NamedTempFile::new().unwrap();
        let path = file.into_temp_path();
        let document = OpmDocument::default();
        assert!(document.save_to_file(&path).is_ok());
        path.close().unwrap()
    }
    #[test]
    fn add_analyzer() {
        let mut document = OpmDocument::default();
        assert!(document.analyzers.is_empty());
        document.add_analyzer(AnalyzerType::Energy);
        assert_eq!(document.analyzers.len(), 1);
    }
    #[test]
    fn analyzers() {
        let mut document = OpmDocument::default();
        document.add_analyzer(AnalyzerType::Energy);
        document.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
        assert_eq!(document.analyzers().len(),2);
    }
}
