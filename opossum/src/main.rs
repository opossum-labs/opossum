use clap::Parser;
use opossum::error::OpmResult;
use opossum::reporter::ReportGenerator;
use opossum::{
    OpticScenery,
    {
        console::{Args, PartialArgs},
        error::OpossumError,
    },
};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn read_and_parse_model(path: &Path) -> OpmResult<OpticScenery> {
    print!("\nReading model...");
    let _ = io::stdout().flush();
    let contents = fs::read_to_string(path).map_err(|e| {
        OpossumError::Console(format!("cannot read file {} : {}", path.display(), e))
    })?;
    let scenery: OpticScenery = serde_json::from_str(&contents)
        .map_err(|e| OpossumError::OpticScenery(format!("parsing of model failed: {e}")))?;
    println!("Success");
    Ok(scenery)
}

fn create_dot_or_report_file_instance(
    path: &PathBuf,
    f_name: &str,
    f_ext: &str,
    print_str: &str
) -> OpmResult<File> {
    let mut f_path = path.clone();
    f_path.push(f_name);
    f_path.set_extension(f_ext);
    print!("Write {print_str} to {}...", f_path.display());
    let _ = io::stdout().flush();

    let output = File::create(f_path)
        .map_err(|e| OpossumError::Other(format!("{f_name} file creation failed: {e}")));

    output
}

fn create_dot_file(
    dot_path: &PathBuf,
    fname: &str,
    scenery: &OpticScenery
) -> OpmResult<()>{

    let mut output = create_dot_or_report_file_instance(
        dot_path,
        fname,
        "dot",
        "diagram"
    )?;

    write!(output, "{}", scenery.to_dot("LR")?)
        .map_err(|e| OpossumError::Other(format!("writing dot file failed: {e}")))?;
    println!("Success");
    Ok(())
}

fn create_report_file(
    report_directory: &PathBuf,
    fname: &str,
    scenery: &OpticScenery
) -> OpmResult<()>{

    let mut output = create_dot_or_report_file_instance(
        report_directory,
        fname,
        "json",
        "detector report"
    )?;

    let analysis_report = scenery.report(report_directory)?;
    write!(
        output,
        "{}",
        serde_json::to_string_pretty(&analysis_report).unwrap()
    )
    .map_err(|e| OpossumError::Other(format!("writing report file failed: {e}")))?;
    let pdf_generator = ReportGenerator::new(analysis_report);
    let mut report_path = report_directory.clone();
    report_path.push("report.pdf");
    pdf_generator.generate_pdf(&report_path)?;
    println!("Success");
    Ok(())
}

fn main() -> OpmResult<()> {
    let opossum_args = Args::try_from(PartialArgs::parse())?;
    let mut scenery = read_and_parse_model(&opossum_args.file_path)?;

    create_dot_file(&opossum_args.report_directory, opossum_args
        .file_path
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap(), &scenery)?;

    scenery.analyze(&opossum_args.analyzer)?;

    create_report_file(
        &opossum_args.report_directory,
        "report",
        &scenery
    )
}
// fn main() -> OpmResult<()> {
//     let opossum_args = Args::try_from(PartialArgs::parse())?;
//     let mut scenery = read_and_parse_model(&opossum_args.file_path)?;

//     let mut dot_path = opossum_args.report_directory.clone();
//     dot_path.push(opossum_args.file_path.file_stem().unwrap());
//     dot_path.set_extension("dot");
//     print!("Write diagram to {}...", dot_path.display());
//     let _ = io::stdout().flush();
//     let mut output = File::create(dot_path)
//         .map_err(|e| OpossumError::Other(format!("dot file creation failed: {e}")))?;
//     write!(output, "{}", scenery.to_dot("LR")?)
//         .map_err(|e| OpossumError::Other(format!("writing dot file failed: {e}")))?;
//     println!("Success");
//     print!("\nAnalyzing...");
//     let _ = io::stdout().flush();
//     scenery.analyze(&opossum_args.analyzer)?;
//     println!("Success\n");
//     let mut report_path = opossum_args.report_directory.clone();
//     report_path.push("report.json");
//     print!("Write detector report to {}...", report_path.display());
//     let _ = io::stdout().flush();
//     let mut output = File::create(report_path)
//         .map_err(|e| OpossumError::Other(format!("report file creation failed: {e}")))?;
//     let analysis_report = scenery.report(&opossum_args.report_directory)?;
//     write!(
//         output,
//         "{}",
//         serde_json::to_string_pretty(&analysis_report).unwrap()
//     )
//     .map_err(|e| OpossumError::Other(format!("writing report file failed: {e}")))?;
//     let pdf_generator = ReportGenerator::new(analysis_report);
//     let mut report_path = opossum_args.report_directory.clone();
//     report_path.push("report.pdf");
//     pdf_generator.generate_pdf(&report_path)?;
//     println!("Success");
//     Ok(())
// }
