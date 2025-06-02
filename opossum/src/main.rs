//! Main function of Opossum
#![warn(missing_docs)]
use clap::Parser;
use env_logger::Env;
use log::{error, info, warn};
use opossum::reporting::analysis_report::AnalysisReport;
use opossum::{
    OpmDocument,
    console::{Args, PartialArgs},
    error::{OpmResult, OpossumError},
    nodes::NodeGroup,
};
use std::{
    env,
    fs::{File, create_dir, remove_dir_all},
    io::{self, Write},
    path::{Path, PathBuf},
};

fn read_and_parse_model(path: &Path) -> OpmResult<OpmDocument> {
    info!("Reading model...");
    OpmDocument::from_file(path)
}

fn create_f_path(path: &Path, f_name: &str, f_ext: &str) -> PathBuf {
    let mut f_path = path.to_path_buf();
    f_path.push(f_name);
    f_path.set_extension(f_ext);
    f_path
}

fn create_dot_or_report_file_instance(
    path: &Path,
    f_name: &str,
    f_ext: &str,
    print_str: &str,
) -> OpmResult<File> {
    let f_path = create_f_path(path, f_name, f_ext);

    info!("Write {print_str} to {}...", f_path.display());
    let _ = io::stdout().flush();

    File::create(f_path)
        .map_err(|e| OpossumError::Other(format!("{f_name} fdile creation failed: {e}")))
}

fn create_dot_file(dot_path: &Path, scenery: &NodeGroup) -> OpmResult<()> {
    let mut output = create_dot_or_report_file_instance(dot_path, "scenery", "dot", "diagram")?;

    write!(output, "{}", scenery.toplevel_dot("")?)
        .map_err(|e| OpossumError::Other(format!("writing diagram file (.dot) failed: {e}")))?;

    let mut output = create_dot_or_report_file_instance(dot_path, "scenery", "svg", "diagram")?;

    let f_path = create_f_path(dot_path, "scenery", "dot");
    scenery.toplevel_dot_svg(&f_path, &mut output)?;

    Ok(())
}
fn create_data_dir(report_directory: &Path) -> OpmResult<()> {
    let data_dir = report_directory.join("data/");
    if data_dir.exists() {
        info!("Delete old report data dir");
        remove_dir_all(&data_dir)
            .map_err(|e| OpossumError::Other(format!("removing old data directory failed: {e}")))?;
    }
    create_dir(&data_dir)
        .map_err(|e| OpossumError::Other(format!("creating data directory failed: {e}")))
}
fn create_report_and_data_files(
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

fn opossum() -> OpmResult<()> {
    // by default, log everything from level `info` and up.
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    warn!(
        "Current work dir: {}",
        env::current_dir().unwrap().display()
    );
    // parse CLI arguments
    let opossum_args = Args::try_from(PartialArgs::parse())?;

    // read scenery model from file and deserialize it
    let mut document = read_and_parse_model(&opossum_args.file_path)?;
    // create the dot file of the scenery
    create_data_dir(&opossum_args.report_directory)?;
    create_dot_file(&opossum_args.report_directory, document.scenery())?;
    let reports = document.analyze()?;
    for report in reports.iter().enumerate() {
        create_report_and_data_files(&opossum_args.report_directory, report.1, report.0)?;
    }
    Ok(())
}
/// OPOSSUM main function
///
/// This function is only a wrapper for the `opossum()` function and does general error handling.
fn main() {
    if let Err(e) = opossum() {
        error!("{e}");
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use std::fs;

    #[test]
    fn create_dot_file_test() {
        let mut document =
            read_and_parse_model(&Path::new("./files_for_testing/opm/opticscenery.opm")).unwrap();
        let scenery = document.scenery_mut();
        let dot_file = create_dot_file(&Path::new("./files_for_testing/dot/_not_valid/"), &scenery);
        assert!(dot_file.is_err());
        let _ = create_dot_file(&Path::new("./files_for_testing/dot/"), &scenery).unwrap();
        fs::remove_file("./files_for_testing/dot/scenery.dot").unwrap();
        fs::remove_file("./files_for_testing/dot/scenery.svg").unwrap();
    }
    #[test]
    fn create_report_file_test() {
        let mut document =
            read_and_parse_model(&Path::new("./files_for_testing/opm/opticscenery.opm")).unwrap();
        let reports = document.analyze().unwrap();
        let report_file = create_report_and_data_files(
            &Path::new("./files_for_testing/report/_not_valid/"),
            &reports[0],
            0,
        );
        assert!(report_file.is_err());
    }
}
