use directories::{ProjectDirs, UserDirs};
use opossum_core::{
    error::{OpmResult, OpossumError},
    nanometer,
};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use uom::si::f64::Length;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct AppConfig {
    report_dir: Option<PathBuf>,
    catalog_dir: Option<PathBuf>,
    default_wavelength: Length,
}

impl Default for AppConfig {
    fn default() -> Self {
        let report_base_dir = UserDirs::new()
            .and_then(|user_dirs| user_dirs.document_dir().map(|p| p.join("opossum_reports")));

        // Default catalog directory in the local config directory: <project dir>/catalogs
        let catalog_base_dir = ProjectDirs::from("org", "Opossumlabs", "Opossum")
            .map(|project_dirs| project_dirs.data_local_dir().join("catalogs"));

        Self {
            report_dir: report_base_dir,
            catalog_dir: catalog_base_dir,
            default_wavelength: nanometer!(1053.0),
        }
    }
}

impl AppConfig {
    pub fn from_file() -> OpmResult<Self> {
        if let Some(config_file) = Self::config_file() {
            let contents = fs::read_to_string(&config_file).map_err(|e| {
                OpossumError::OpmDocument(format!(
                    "cannot read file {} : {}",
                    config_file.display(),
                    e
                ))
            })?;
            let app_config: Self = ron::from_str(&contents).map_err(|e| {
                OpossumError::OpmDocument(format!("parsing of config file failed: {e}"))
            })?;
            Ok(app_config)
        } else {
            Err(OpossumError::Other("config file not found".into()))
        }
    }

    pub fn to_file(&self) -> OpmResult<()> {
        let config = PrettyConfig::new().new_line("\n");
        let serialized = ron::ser::to_string_pretty(&self, config).map_err(|e| {
            OpossumError::OpticScenery(format!("serialization of config failed: {e}"))
        })?;
        let Some(config_file) = Self::config_file() else {
            return Err(OpossumError::Other(
                "could not determine config file name".into(),
            ));
        };
        // Create parent directories if not present
        if let Some(parent) = config_file.parent() {
            fs::create_dir_all(parent).ok();
        }
        let mut output = File::create(&config_file).map_err(|e| {
            OpossumError::OpticScenery(format!(
                "could not create file path: {}: {}",
                config_file.display(),
                e
            ))
        })?;
        write!(output, "{serialized}").map_err(|e| {
            OpossumError::OpticScenery(format!(
                "writing to file path {} failed: {}",
                config_file.display(),
                e
            ))
        })?;
        Ok(())
    }

    pub const fn report_dir(&self) -> Option<&PathBuf> {
        self.report_dir.as_ref()
    }

    pub fn set_report_dir(&mut self, report_dir: &Path) -> OpmResult<()> {
        if !report_dir.is_dir() {
            return Err(OpossumError::Other(format!(
                "error setting report directory: {}",
                report_dir.to_path_buf().display()
            )));
        }
        self.report_dir = Some(report_dir.to_path_buf());
        Ok(())
    }

    pub const fn catalog_dir(&self) -> Option<&PathBuf> {
        self.catalog_dir.as_ref()
    }

    pub fn set_catalog_dir(&mut self, catalog_dir: &Path) -> OpmResult<()> {
        if !catalog_dir.is_dir() {
            return Err(OpossumError::Other(format!(
                "error setting catalog directory: {}",
                catalog_dir.to_path_buf().display()
            )));
        }
        self.catalog_dir = Some(catalog_dir.to_path_buf());
        Ok(())
    }

    fn config_file() -> Option<PathBuf> {
        ProjectDirs::from("org", "Opossumlabs", "Opossum")
            .map(|project_dirs| project_dirs.config_local_dir().join("config.ron"))
    }
}
