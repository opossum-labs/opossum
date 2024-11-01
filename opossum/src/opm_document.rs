use crate::{
    analyzers::AnalyzerType,
    error::{OpmResult, OpossumError},
    nodes::NodeGroup,
    optic_node::OpticNode,
    SceneryResources,
};
use log::warn;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    fs::{self, File},
    io::Write,
    path::Path,
    rc::Rc,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct OpmDocument {
    #[serde(rename = "opm file version")]
    opm_file_version: String,
    #[serde(default)]
    scenery: NodeGroup,
    #[serde(default, rename = "global")]
    global_conf: Rc<RefCell<SceneryResources>>,
    #[serde(default)]
    analyzers: Vec<AnalyzerType>,
}
impl Default for OpmDocument {
    fn default() -> Self {
        Self {
            opm_file_version: env!("OPM_FILE_VERSION").to_string(),
            scenery: NodeGroup::default(),
            global_conf: Rc::new(RefCell::new(SceneryResources::default())),
            analyzers: vec![],
        }
    }
}
impl OpmDocument {
    /// Creates a new [`OpmDocument`].
    #[must_use]
    pub fn new(scenery: NodeGroup) -> Self {
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
        let mut document: Self = serde_yaml::from_str(&contents)
            .map_err(|e| OpossumError::OpmDocument(format!("parsing of model failed: {e}")))?;
        if document.opm_file_version != env!("OPM_FILE_VERSION") {
            warn!("OPM file version does not match the used OPOSSUM version.");
            warn!(
                "read version '{}' <-> program file version '{}'",
                document.opm_file_version,
                env!("OPM_FILE_VERSION")
            );
            warn!("This file might haven been written by an older or newer version of OPOSSUM. The model import might not be correct.");
        }
        document.scenery.after_deserialization_hook()?;
        document
            .scenery
            .graph_mut()
            .update_global_config(&Some(document.global_conf.clone()));
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
            OpossumError::OpticScenery(format!("serialization of OpmDocument failed: {e}"))
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
    pub fn scenery_mut(&mut self) -> &mut NodeGroup {
        &mut self.scenery
    }
    #[must_use]
    pub fn analyzers(&self) -> Vec<AnalyzerType> {
        self.analyzers.clone()
    }
    /// Returns a reference to the global config of this [`OpmDocument`].
    #[must_use]
    pub fn global_conf(&self) -> &RefCell<SceneryResources> {
        &self.global_conf
    }
    /// Sets the global config of this [`OpmDocument`].
    pub fn set_global_conf(&mut self, rsrc: SceneryResources) {
        self.global_conf = Rc::new(RefCell::new(rsrc));
        self.scenery
            .graph_mut()
            .update_global_config(&Some(self.global_conf.clone()));
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{analyzers::RayTraceConfig, optic_node::OpticNode};
    use petgraph::adj::NodeIndex;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    #[test]
    fn new() {
        let mut scenery = NodeGroup::default();
        scenery.node_attr_mut().set_name("MyTest");
        let document = OpmDocument::new(scenery);
        assert_eq!(document.scenery.node_attr().name(), "MyTest");
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
            "d8c43481-43b7-4bad-8305-7246a62e4016",
            node1.uuid().to_string()
        );
        assert_eq!(
            "e6729ff1-e14a-468d-a08f-d0b9dd131dab",
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
        assert_eq!(document.analyzers().len(), 2);
    }
}
