//! Main function of Opossum
#![warn(missing_docs)]
use clap::Parser;
use env_logger::Env;
use log::{error, info, warn};
use opossum::{
    OpmDocument,
    console::{Args, PartialArgs},
    error::{OpmResult, OpossumError},
    nodes::NodeGroup,
};
use opossum::{
    create_data_dir, create_dot_or_report_file_instance, create_f_path,
    create_report_and_data_files,
};
use std::{env, io::Write, path::Path};

fn read_and_parse_model(path: &Path) -> OpmResult<OpmDocument> {
    info!("Reading model...");
    OpmDocument::from_file(path)
}

fn create_dot_file(dot_path: &Path, scenery: &NodeGroup) -> OpmResult<()> {
    let mut output = create_dot_or_report_file_instance(dot_path, "scenery", "dot", "diagram")?;

    write!(output, "{}", scenery.toplevel_dot("")?)
        .map_err(|e| OpossumError::Other(format!("writing diagram file (.dot) failed: {e}")))?;

    let mut output = create_dot_or_report_file_instance(dot_path, "scenery", "svg", "diagram")?;

    let f_path = create_f_path(dot_path, "scenery", "dot");
    scenery
        .toplevel_dot_svg(&f_path, &mut output)
        .unwrap_or_else(|e| {
            warn!("Creating SVG file failed: {e}");
        });

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
    use opossum::create_report_and_data_files;

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
