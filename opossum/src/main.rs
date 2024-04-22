//! Mainf function of Opossum
#![warn(missing_docs)]
use clap::Parser;
use env_logger::Env;
use log::{error, info};
use opossum::{
    analyzer::AnalyzerType,
    console::{Args, PartialArgs},
    error::{OpmResult, OpossumError},
    reporter::{AnalysisReport, ReportGenerator},
    OpticScenery,
    bevy_main
};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

fn read_and_parse_model(path: &Path) -> OpmResult<OpticScenery> {
    info!("Reading model...");
    let _ = io::stdout().flush();
    let contents = fs::read_to_string(path).map_err(|e| {
        OpossumError::Console(format!("cannot read file {} : {}", path.display(), e))
    })?;
    let scenery: OpticScenery = serde_yaml::from_str(&contents)
        .map_err(|e| OpossumError::OpticScenery(format!("parsing of model failed: {e}")))?;
    Ok(scenery)
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
        .map_err(|e| OpossumError::Other(format!("{f_name} file creation failed: {e}")))
}

fn create_dot_file(dot_path: &Path, fname: &str, scenery: &OpticScenery) -> OpmResult<()> {
    let mut output = create_dot_or_report_file_instance(dot_path, fname, "dot", "diagram")?;

    write!(output, "{}", scenery.to_dot("LR")?)
        .map_err(|e| OpossumError::Other(format!("writing dot file failed: {e}")))?;

    let mut output = create_dot_or_report_file_instance(dot_path, fname, "svg", "diagram")?;
    write!(output, "{}", scenery.to_dot_svg()?)
        .map_err(|e| OpossumError::Other(format!("writing dot file failed: {e}")))?;
    Ok(())
}

fn create_report_file(
    report_directory: &Path,
    base_file_name: &str,
    scenery: &OpticScenery,
    analyzer: &AnalyzerType,
) -> OpmResult<AnalysisReport> {
    let mut output =
        create_dot_or_report_file_instance(report_directory, "report", "yaml", "detector report")?;
    let analysis_report = scenery.report(report_directory)?;
    write!(
        output,
        "{}",
        serde_yaml::to_string(&analysis_report).unwrap()
    )
    .map_err(|e| OpossumError::Other(format!("writing report file failed: {e}")))?;
    let report_generator = ReportGenerator::new(analysis_report.clone(), Path::new(base_file_name));
    let mut report_path = report_directory.to_path_buf();
    report_path.push("report.html");
    report_generator.generate_html(&report_path, analyzer)?;
    Ok(analysis_report)
}

fn opossum() -> OpmResult<()> {
    // by default, log everything from level `info` and up.
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    //parse CLI arguments
    let opossum_args = Args::try_from(PartialArgs::parse())?;

    //read scenery model from file and deserialize it
    let mut scenery = read_and_parse_model(&opossum_args.file_path)?;

    let base_file_name = opossum_args
        .file_path
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap();

    //create the dot file of the scenery
    create_dot_file(&opossum_args.report_directory, base_file_name, &scenery)?;
    //analyze the scenery
    info!("Analyzing...");
    scenery.analyze(&opossum_args.analyzer)?;
    let analysis_report = create_report_file(
        &opossum_args.report_directory,
        base_file_name,
        &scenery,
        &opossum_args.analyzer,
    )?;
    info!("Starting visualization");
    bevy_main::bevy_main(&analysis_report);
    Ok(())
}

/// OPOSSUM main function
///
/// This function is only a wrapper for the `opossum()` function and does general erro handling.
fn main() {
    if let Err(e) = opossum() {
        error!("{e}");
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use petgraph::adj::NodeIndex;
    use std::path::PathBuf;

    #[test]
    fn read_and_parse_model_test() {
        assert!(read_and_parse_model(&PathBuf::from(
            "./invalid_file_path/invalid_file.invalid_ext"
        ))
        .is_err());
        assert!(
            read_and_parse_model(&PathBuf::from("./files_for_testing/opm/incorrect_opm.opm"))
                .is_err()
        );

        let scenery =
            read_and_parse_model(&PathBuf::from("./files_for_testing/opm/opticscenery.opm"))
                .unwrap();
        let node1 = scenery.node(NodeIndex::from(0)).unwrap();
        let node2 = scenery.node(NodeIndex::from(1)).unwrap();
        assert_eq!(
            "2ac550f7-b62c-4aa8-8f57-9931a791bc99",
            node1.uuid().to_string()
        );
        assert_eq!(
            "710f252d-2cbd-4613-8135-291a07cd4cbd",
            node2.uuid().to_string()
        );
    }

    #[test]
    fn create_dot_file_test() {
        let scenery =
            read_and_parse_model(&PathBuf::from("./files_for_testing/opm/opticscenery.opm"))
                .unwrap();
        let dot_file = create_dot_file(
            &PathBuf::from("./files_for_testing/dot/_not_valid/"),
            "create_dot_file_test",
            &scenery,
        );
        assert!(dot_file.is_err());
        let _ = create_dot_file(
            &PathBuf::from("./files_for_testing/dot/"),
            "create_dot_file_test",
            &scenery,
        )
        .unwrap();
        fs::remove_file("./files_for_testing/dot/create_dot_file_test.dot").unwrap();
        fs::remove_file("./files_for_testing/dot/create_dot_file_test.svg").unwrap();
    }
    #[test]
    fn create_report_file_test() {
        let scenery =
            read_and_parse_model(&PathBuf::from("./files_for_testing/opm/opticscenery.opm"))
                .unwrap();

        let report_file = create_report_file(
            &PathBuf::from("./files_for_testing/report/_not_valid/"),
            "create_report_file_test",
            &scenery,
            &AnalyzerType::Energy,
        );
        assert!(report_file.is_err());
    }
}
