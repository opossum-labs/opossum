//! Mainf function of Opossum
#![warn(missing_docs)]
use clap::Parser;
use env_logger::Env;
use log::{error, info, warn};
#[cfg(feature = "bevy")]
use opossum::bevy_main;
#[cfg(feature = "bevy")]
use opossum::SceneryBevyData;
use opossum::{
    console::{Args, PartialArgs},
    error::{OpmResult, OpossumError},
    OpticScenery,
};
use std::env;
use std::fs::create_dir;
use std::fs::remove_dir_all;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

fn read_and_parse_model(path: &Path) -> OpmResult<OpticScenery> {
    info!("Reading model...");
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

fn create_dot_file(dot_path: &Path, scenery: &OpticScenery) -> OpmResult<()> {
    let mut output = create_dot_or_report_file_instance(dot_path, "scenery", "dot", "diagram")?;

    write!(output, "{}", scenery.to_dot("")?)
        .map_err(|e| OpossumError::Other(format!("writing diagram file (.dot) failed: {e}")))?;

    let mut output = create_dot_or_report_file_instance(dot_path, "scenery", "svg", "diagram")?;
    write!(output, "{}", scenery.to_dot_svg()?)
        .map_err(|e| OpossumError::Other(format!("writing diagram file (.svg) failed: {e}")))?;

    Ok(())
}
fn create_report_and_data_files(report_directory: &Path, scenery: &OpticScenery) -> OpmResult<()> {
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
    let analysis_report = scenery.report()?;
    write!(
        output,
        "{}",
        serde_yaml::to_string(&analysis_report).unwrap()
    )
    .map_err(|e| OpossumError::Other(format!("writing report file failed: {e}")))?;
    let mut report_path = report_directory.to_path_buf();
    report_path.push("report.html");
    analysis_report.generate_html(&report_path)?;
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
    let mut scenery = read_and_parse_model(&opossum_args.file_path)?;

    //create the dot file of the scenery
    create_dot_file(&opossum_args.report_directory, &scenery)?;
    //analyze the scenery
    info!("Analyzing...");
    scenery.analyze(&opossum_args.analyzer)?;
    #[cfg(feature = "bevy")]
    let analysis_report = create_report_and_data_files(
        &opossum_args.report_directory,
        base_file_name,
        &scenery,
        &opossum_args.analyzer,
    )?;
    #[cfg(not(feature = "bevy"))]
    create_report_and_data_files(&opossum_args.report_directory, &scenery)?;
    #[cfg(feature = "bevy")]
    bevy_main::bevy_main(SceneryBevyData::from_report(&analysis_report));
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
            "180328fe-7ad4-4568-b501-183b88c4daee",
            node1.uuid().to_string()
        );
        assert_eq!(
            "642ce76e-b071-43c0-a77e-1bdbb99b40d8",
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
            &scenery,
        );
        assert!(dot_file.is_err());
        let _ = create_dot_file(&PathBuf::from("./files_for_testing/dot/"), &scenery).unwrap();
        fs::remove_file("./files_for_testing/dot/scenery.dot").unwrap();
        fs::remove_file("./files_for_testing/dot/scenery.svg").unwrap();
    }
    #[test]
    fn create_report_file_test() {
        let scenery =
            read_and_parse_model(&PathBuf::from("./files_for_testing/opm/opticscenery.opm"))
                .unwrap();

        let report_file = create_report_and_data_files(
            &PathBuf::from("./files_for_testing/report/_not_valid/"),
            &scenery,
        );
        assert!(report_file.is_err());
    }
}
