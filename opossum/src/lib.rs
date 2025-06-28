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
pub mod console;
pub mod dottable;
pub mod energy_distributions;
pub mod error;
pub mod fluence_distributions;
mod light_flow;
pub mod light_result;
pub mod lightdata;
pub mod nodes;
pub mod opm_document;
pub mod optic_node;
pub mod optic_ports;
pub mod optic_ref;
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
pub mod coatings;
pub mod kde;
pub mod reporting;
pub mod spectrum;
pub mod spectrum_helper;
pub mod surface;
pub mod utils;
use std::{
    fs::{File, create_dir, remove_dir_all},
    io::{self, Write},
    path::{Path, PathBuf},
};

use chrono::DateTime;
use log::info;
pub use opm_document::OpmDocument;
pub use optic_scenery_rsc::SceneryResources;

use crate::{
    error::{OpmResult, OpossumError},
    reporting::analysis_report::AnalysisReport,
};
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

/// Creates and writes report files in RON and HTML formats, and exports associated data.
///
/// This function serializes the report to a RON file and generates a corresponding HTML report file.
/// It also calls the report’s data export function.
///
/// # Parameters
///
/// * `report_directory` - Path to the directory where the files will be written.
/// * `report` - The `AnalysisReport` to be saved.
/// * `report_number` - A unique number appended to the filename to distinguish multiple reports.
///
/// # Returns
///
/// * `Ok(())` on success.
/// * `Err(OpossumError)` if writing to any file or exporting data fails.
///
/// # Errors
///
/// * Writing the RON or HTML report may fail due to file permission or I/O errors.
/// * Exporting the report data may also return an error from the `AnalysisReport::export_data` implementation.
pub fn create_report_and_data_files(
    report_directory: &Path,
    report: &AnalysisReport,
    report_number: usize,
) -> OpmResult<()> {
    let mut output = create_dot_or_report_file_instance(
        report_directory,
        &format!("report_{report_number}"),
        "ron",
        "analysis report",
    )?;
    write!(output, "{}", report.to_file_string()?)
        .map_err(|e| OpossumError::Other(format!("writing report file failed: {e}")))?;

    let mut report_path = report_directory.to_path_buf();
    report.export_data(&report_path)?;

    report_path.push(format!("report_{report_number}.html"));
    info!("Write html report to {}", report_path.display());
    report.to_html_report()?.generate_html(&report_path)?;

    Ok(())
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
pub fn create_f_path(path: &Path, f_name: &str, f_ext: &str) -> PathBuf {
    let mut f_path = path.to_path_buf();
    f_path.push(f_name);
    f_path.set_extension(f_ext);
    f_path
}


/// Creates a new file at a path constructed from directory, filename, and extension,
/// and logs the operation with a user-defined description.
///
/// # Parameters
///
/// * `path` - Base directory for the file.
/// * `f_name` - Name of the file without extension.
/// * `f_ext` - File extension (e.g., `"ron"`, `"dot"`).
/// * `print_str` - A descriptive string used for logging (e.g., `"analysis report"`).
///
/// # Returns
///
/// * `Ok(File)` if the file is successfully created.
/// * `Err(OpossumError)` if file creation fails.
///
/// # Errors
///
/// Returns an error if the file cannot be created (e.g., due to permissions, invalid path, or I/O issues).
pub fn create_dot_or_report_file_instance(
    path: &Path,
    f_name: &str,
    f_ext: &str,
    print_str: &str,
) -> OpmResult<File> {
    let f_path = create_f_path(path, f_name, f_ext);

    info!("Write {print_str} to {}...", f_path.display());
    let _ = io::stdout().flush();

    File::create(f_path)
        .map_err(|e| OpossumError::Other(format!("{f_name} file creation failed: {e}")))
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
