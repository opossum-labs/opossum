//! Helper functions for exporting [`AnalysisReport`]s to disk.

use std::{fs::File, io::Write, path::Path};

use log::info;

use crate::{
    create_f_path,
    error::{OpmResult, OpossumError},
    reporting::analysis_report::AnalysisReport,
};

/// Creates and writes report files in RON and HTML formats, and exports associated data.
///
/// This function serializes the report to a RON file and generates a corresponding HTML report file.
/// It also calls the report’s data export function.
///
/// # Parameters
///
/// * `report_directory` - Path to the directory where the files will be written.
/// * `report` - The [`AnalysisReport`] to be saved.
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
    let mut output =
        create_file_instance(report_directory, &format!("report_{report_number}"), "ron")?;
    write!(output, "{}", report.to_file_string()?)
        .map_err(|e| OpossumError::Other(format!("writing report file failed: {e}")))?;

    let mut report_path = report_directory.to_path_buf();
    report.export_data(&report_path)?;

    report_path.push(format!("report_{report_number}.html"));
    info!("Write html report to {}", report_path.display());
    report.to_html_report()?.generate_html(&report_path)?;
    Ok(())
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
pub fn create_file_instance(path: &Path, f_name: &str, f_ext: &str) -> OpmResult<File> {
    let f_path = create_f_path(path, f_name, f_ext);
    File::create(f_path)
        .map_err(|e| OpossumError::Other(format!("{f_name} file creation failed: {e}")))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::OpmDocument;

    #[test]
    fn create_report_file_test() {
        let mut document =
            OpmDocument::from_file(&Path::new("./files_for_testing/opm/opticscenery.opm")).unwrap();
        let reports = document.analyze().unwrap();
        let report_file = create_report_and_data_files(
            &Path::new("./files_for_testing/report/_not_valid/"),
            &reports[0],
            0,
        );
        assert!(report_file.is_err());
    }
}
