//! Mainf function of Opossum
#![warn(missing_docs)]
use clap::Parser;
use env_logger::Env;
use log::{error, info, warn};
use opossum::analyzers::energy::EnergyAnalyzer;
use opossum::analyzers::ghostfocus::GhostFocusAnalyzer;
use opossum::analyzers::raytrace::RayTracingAnalyzer;
use opossum::analyzers::Analyzer;
use opossum::analyzers::AnalyzerType;
#[cfg(feature = "bevy")]
use opossum::bevy_main;
use opossum::nodes::NodeGroup;
use opossum::optic_node::OpticNode;
use opossum::OpmDocument;
#[cfg(feature = "bevy")]
use opossum::SceneryBevyData;
use opossum::{
    console::{Args, PartialArgs},
    error::{OpmResult, OpossumError},
};
use std::env;
use std::fs::create_dir;
use std::fs::remove_dir_all;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

fn read_and_parse_model(path: &Path) -> OpmResult<OpmDocument> {
    info!("Reading model...");
    OpmDocument::from_file(path)
}

fn create_dot_or_report_file_instance(
    path: &Path,
    f_name: &str,
    f_ext: &str,
    print_str: &str,
) -> OpmResult<File> {
    let mut f_path = path.to_path_buf();
    f_path.push(f_name);
    f_path.set_extension(f_ext);
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
    write!(output, "{}", scenery.toplevel_dot_svg()?)
        .map_err(|e| OpossumError::Other(format!("writing diagram file (.svg) failed: {e}")))?;
    Ok(())
}
fn create_report_and_data_files(
    report_directory: &Path,
    analyzer: &dyn Analyzer,
    scenery: &NodeGroup,
) -> OpmResult<()> {
    let data_dir = report_directory.join("data/");
    if data_dir.exists() {
        info!("Delete old report data dir");
        remove_dir_all(&data_dir)
            .map_err(|e| OpossumError::Other(format!("removing old data directory failed: {e}")))?;
    }
    create_dir(&data_dir)
        .map_err(|e| OpossumError::Other(format!("creating data directory failed: {e}")))?;
    scenery.export_node_data(&data_dir)?;
    let mut output =
        create_dot_or_report_file_instance(report_directory, "report", "yaml", "detector report")?;
    let analysis_report = analyzer.report(scenery)?;
    write!(
        output,
        "{}",
        serde_yaml::to_string(&analysis_report).unwrap()
    )
    .map_err(|e| OpossumError::Other(format!("writing report file failed: {e}")))?;
    let mut report_path = report_directory.to_path_buf();
    report_path.push("report.html");
    analysis_report
        .to_html_report()?
        .generate_html(&report_path)?;
    scenery.export_data(&data_dir, "")?;
    Ok(())
}

fn opossum() -> OpmResult<()> {
    // by default, log everything from level `info` and up.
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    warn!(
        "Current work dir: {}",
        env::current_dir().unwrap().display()
    );
    //parse CLI arguments
    let opossum_args = Args::try_from(PartialArgs::parse())?;

    //read scenery model from file and deserialize it
    let mut document = read_and_parse_model(&opossum_args.file_path)?;
    let analyzers = document.analyzers();
    let scenery = document.scenery_mut();
    //create the dot file of the scenery
    create_dot_file(&opossum_args.report_directory, scenery)?;
    if analyzers.is_empty() {
        info!("No analyzer defined in document. Stopping here.");
    } else {
        info!("Analyzing...");
        for ana in &analyzers {
            let analyzer: &dyn Analyzer = match ana {
                AnalyzerType::Energy => &EnergyAnalyzer::default(),
                AnalyzerType::RayTrace(config) => &RayTracingAnalyzer::new(config.clone()),
                AnalyzerType::GhostFocus(config) => &GhostFocusAnalyzer::new(config.clone()),
                _ => {
                    return Err(OpossumError::Analysis(
                        "specified analyzer not found".into(),
                    ))
                }
            };
            analyzer.analyze(scenery)?;
            #[cfg(feature = "bevy")]
            let analysis_report = create_report_and_data_files(
                &opossum_args.report_directory,
                base_file_name,
                &scenery,
                &opossum_args.analyzer,
            )?;
            #[cfg(not(feature = "bevy"))]
            create_report_and_data_files(&opossum_args.report_directory, analyzer, scenery)?;
            #[cfg(feature = "bevy")]
            bevy_main::bevy_main(SceneryBevyData::from_report(&analysis_report));
        }
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
        let scenery = document.scenery_mut();
        let report_file = create_report_and_data_files(
            &Path::new("./files_for_testing/report/_not_valid/"),
            &EnergyAnalyzer::default(),
            &scenery,
        );
        assert!(report_file.is_err());
    }
}
