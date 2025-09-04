//! ![OPOSSUM logo][opossum_logo]
//!
//! This is the documentation for the **OPOSSUM** software package. **OPOSSUM** stands for
//! **Op**en-source **O**ptics **S**imulation **S**oftware and **U**nified **M**odeler.
//!
#![cfg_attr(feature = "doc-images",
cfg_attr(all(),
doc = ::embed_doc_image::embed_image!("opossum_logo", "logo/Logo_text.svg")))]
#![cfg_attr(
    not(feature = "doc-images"),
    doc = "**Doc images not enabled**. Compile with feature `doc-images` and Rust version >= 1.54 \
           to enable."
)]
#![allow(clippy::module_name_repetitions)]

pub mod analyzers;
pub mod aperture;
pub mod coatings;
pub mod energy_distributions;
pub mod error;
pub mod fluence_distributions;
mod dottable;
mod kde;
mod light_flow;
mod light_result;
pub mod lightdata;
pub mod nodes;
mod opm_document;
pub mod optic_node;
pub mod optic_ports;
mod optic_ref;
mod optic_scenery_rsc;
pub mod plottable;
mod port_map;
pub mod position_distributions;
pub mod properties;
pub mod ray;
pub mod rays;
pub mod refractive_index;
pub mod spectral_distribution;
// pub mod render;
pub mod reporting;
pub mod spectrum;
pub mod spectrum_helper;
pub mod surface;
pub mod utils;
use std::{
    fs::{create_dir, remove_dir_all},
    path::{Path, PathBuf},
};

use chrono::DateTime;
use log::info;
pub use opm_document::{AnalyzerInfo, OpmDocument};
pub use optic_scenery_rsc::SceneryResources;

use crate::error::{OpmResult, OpossumError};
/// Creates a fresh `data/` subdirectory in the given report directory.
///
/// If a `data/` folder already exists, it is deleted first.
///
/// # Parameters
///
/// * `report_directory` - Path to the root directory where the `data/` folder should be created.
///
/// # Returns
///
/// * `Ok(())` if the directory is successfully created.
/// * `Err(OpossumError)` if removing or creating the directory fails.
///
/// # Errors
///
/// * Returns an error if the existing `data/` directory cannot be deleted.
/// * Returns an error if the `data/` directory cannot be created.
pub fn create_data_dir(report_directory: &Path) -> OpmResult<()> {
    let data_dir = report_directory.join("data/");
    if data_dir.exists() {
        info!("Delete old report data dir");
        remove_dir_all(&data_dir)
            .map_err(|e| OpossumError::Other(format!("removing old data directory failed: {e}")))?;
    }
    create_dir(&data_dir)
        .map_err(|e| OpossumError::Other(format!("creating data directory failed: {e}")))
}

/// Constructs a `PathBuf` from a directory, a filename (without extension), and a file extension.
///
/// # Parameters
///
/// * `path` - Base directory path.
/// * `f_name` - Filename without extension.
/// * `f_ext` - File extension to be appended (e.g., `"ron"`, `"html"`).
///
/// # Returns
///
/// A `PathBuf` representing the full file path with the specified extension.
#[must_use]
pub fn create_f_path(path: &Path, f_name: &str, f_ext: &str) -> PathBuf {
    let mut f_path = path.to_path_buf();
    f_path.push(f_name);
    f_path.set_extension(f_ext);
    f_path
}

/// Return the version information of the currently built OPOSSUM executable.
///
/// This function returs a `String` which contains the current Git tag/hash combination as well as
/// the timestamp of this commit.
#[must_use]
pub fn get_version() -> String {
    let timestamp = DateTime::parse_from_rfc3339(env!("VERGEN_GIT_COMMIT_TIMESTAMP")).map_or_else(
        |_| String::from("invalid timestamp"),
        |timestamp| timestamp.format("%Y/%m/%d %H:%M").to_string(),
    );
    format!("{} ({})", env!("VERGEN_GIT_DESCRIBE"), timestamp)
}
#[cfg(test)]
mod test {
    use super::*;
    use regex::Regex;
    #[test]
    #[ignore]
    fn get_ver() {
        let version_string = get_version();
        let re = Regex::new(r"(.*) \(\d{4}/\d{2}/\d{2} \d{2}:\d{2}\)").unwrap();
        assert!(re.is_match(&version_string));
    }
}
