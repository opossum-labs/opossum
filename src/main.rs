use clap::Parser;
use opossum::error::OpmResult;
use opossum::{
    OpticScenery,
    {
        console::{Args, PartialArgs},
        error::OpossumError,
    },
};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

fn read_and_parse_model(path: &Path) -> OpmResult<OpticScenery> {
    print!("\nReading model...");
    let _ = io::stdout().flush();
    let contents = fs::read_to_string(path).map_err(|e| {
        OpossumError::Console(format!("cannot read file {} : {}", path.display(), e))
    })?;
    let scenery: OpticScenery = serde_json::from_str(&contents)
        .map_err(|e| OpossumError::OpticScenery(format!("parsing of model failed: {}", e)))?;
    println!("Success");
    Ok(scenery)
}

fn main() -> OpmResult<()> {
    let opossum_args = Args::try_from(PartialArgs::parse())?;
    let mut scenery = read_and_parse_model(&opossum_args.file_path)?;

    let mut dot_path = opossum_args.report_directory.clone();
    dot_path.push(opossum_args.file_path.file_stem().unwrap());
    dot_path.set_extension("dot");
    print!("Write diagram to {}...", dot_path.display());
    let _ = io::stdout().flush();
    let mut output = File::create(dot_path)
        .map_err(|e| OpossumError::Other(format!("dot file creation failed: {}", e)))?;
    write!(output, "{}", scenery.to_dot("LR")?)
        .map_err(|e| OpossumError::Other(format!("writing dot file failed: {}", e)))?;
    println!("Success");
    print!("\nAnalyzing...");
    let _ = io::stdout().flush();
    scenery.analyze(&opossum_args.analyzer)?;
    println!("Success\n");
    let mut report_path = opossum_args.report_directory.clone();
    report_path.push("report.json");
    print!("Write detector report to {}...", report_path.display());
    let _ = io::stdout().flush();
    let mut output = File::create(report_path)
        .map_err(|e| OpossumError::Other(format!("report file creation failed: {}", e)))?;
    write!(
        output,
        "{}",
        serde_json::to_string_pretty(&scenery.report(&opossum_args.report_directory)).unwrap()
    )
    .map_err(|e| OpossumError::Other(format!("writing report file failed: {}", e)))?;
    println!("Success");
    Ok(())
}
