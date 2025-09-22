//! Main function of Opossum
#![warn(missing_docs)]
use clap::Parser;
use env_logger::Env;
use log::{error, info, warn};
use opossum_core::{
    error::OpmResult, opm_document::OpmDocument, utils::file_utils::recreate_data_dir,
};
use std::{env, path::Path};
mod console;
use crate::console::{Args, PartialArgs};

fn read_and_parse_model(path: &Path) -> OpmResult<OpmDocument> {
    info!("Reading model...");
    OpmDocument::from_file(path)
}
fn opossum() -> OpmResult<()> {
    // by default, log everything from level `info` and up.
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    info!(
        "Current work dir: {}",
        env::current_dir().unwrap().display()
    );
    // parse CLI arguments
    let opossum_args = Args::try_from(PartialArgs::parse())?;

    // read scenery model from file and deserialize it
    let mut document = read_and_parse_model(&opossum_args.file_path)?;
    // create the dot file of the scenery
    recreate_data_dir(&opossum_args.report_directory)?;
    document
        .create_dot_file(&opossum_args.report_directory)
        .unwrap_or_else(|e| warn!("{e}"));
    let reports = document.analyze()?;
    for report in reports.iter().enumerate() {
        report.1.save(&opossum_args.report_directory, report.0)?;
    }
    Ok(())
}
/// OPOSSUM main function
///
/// This function is only a wrapper for the `opossum()` function and does general error handling.
fn main() {
    opossum().unwrap_or_else(|e| error!("{e}"));
}
