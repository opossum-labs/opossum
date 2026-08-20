use directories::{ProjectDirs, UserDirs};
use opm_macros_lib::EnsureValidated;
use opossum_core::{
    error::{OpmResult, OpossumError},
    generic_validators::{AllFinite, AllNotZero, AllPositive, ValidateTrait},
    nanometer, validated, validated_type,
};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use uom::si::f64::Length;

/// Default Git repository URL for syncing catalog data.
pub const DEFAULT_CATALOG_REMOTE_URL: &str = "https://github.com/opossum-labs/opossum_catalog.git";

#[derive(Serialize, Deserialize, Debug, Clone, EnsureValidated)]
#[serde(default)]
pub struct AppConfig {
    #[validate(skip)]
    report_dir: Option<PathBuf>,
    #[validate(skip)]
    catalog_dir: Option<PathBuf>,
    #[validate(skip)]
    catalog_remote_url: String,
    #[validate(skip)]
    sync_catalog_on_startup: bool,

    default_wavelength: validated_type!(Length, AllPositive && AllFinite && AllNotZero),
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
            catalog_remote_url: DEFAULT_CATALOG_REMOTE_URL.to_string(),
            sync_catalog_on_startup: false,
            default_wavelength: validated!(
                nanometer!(1053.0),
                AllPositive && AllFinite && AllNotZero
            )
            .unwrap(),
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

    /// Returns the default wavelength.
    pub const fn default_wavelength(&self) -> Length {
        *self.default_wavelength.get()
    }

    /// Sets the default wavelength.
    pub fn set_default_wavelength(&mut self, wavelength: Length) {
        self.default_wavelength.set(wavelength);
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

    /// Returns the remote Git repository URL for catalog synchronization.
    pub fn catalog_remote_url(&self) -> &str {
        &self.catalog_remote_url
    }

    /// Sets the remote Git repository URL.
    pub fn set_catalog_remote_url(&mut self, url: impl Into<String>) {
        self.catalog_remote_url = url.into().trim().to_string();
    }

    /// Returns whether the catalog should be checked and updated on application startup.
    pub const fn sync_catalog_on_startup(&self) -> bool {
        self.sync_catalog_on_startup
    }

    /// Sets whether the catalog should be checked and updated on application startup.
    pub fn set_sync_catalog_on_startup(&mut self, sync: bool) {
        self.sync_catalog_on_startup = sync;
    }

    fn config_file() -> Option<PathBuf> {
        ProjectDirs::from("org", "Opossumlabs", "Opossum")
            .map(|project_dirs| project_dirs.config_local_dir().join("config.ron"))
    }
}
