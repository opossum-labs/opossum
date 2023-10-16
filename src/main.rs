use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use opossum::get_version;
use clap::{Parser, Command};
use opossum::error::OpmResult;
use opossum::{
    OpticScenery,
    {
        console::{show_intro, Args, PartialArgs},
        error::OpossumError,
    },
};
fn main() {
    //not necessary, just for fun
    show_intro();
    if let Err(e) = do_it() {
        println!("Error: {}", e);
        std::process::exit(1);
    }
}
fn read_and_parse_model(path: &Path) -> OpmResult<OpticScenery> {
    print!("\nReading model...");
    let contents = fs::read_to_string(path).map_err(|e| {
        OpossumError::Console(format!("cannot read file {} : {}", path.display(), e))
    })?;
    let scenery: OpticScenery = serde_json::from_str(&contents)
        .map_err(|e| OpossumError::OpticScenery(format!("parsing of model failed: {}", e)))?;
    println!("Success");
    Ok(scenery)
}

fn do_it() -> OpmResult<()> {
    let opossum_args = Args::try_from(PartialArgs::parse())?;
    let mut scenery = read_and_parse_model(&opossum_args.file_path)?;

    let mut dot_path = opossum_args.report_directory.clone();
    dot_path.push(opossum_args.file_path.file_stem().unwrap());
    dot_path.set_extension("dot");
    print!("Write diagram to {}...", dot_path.display());
    let mut output = File::create(dot_path).unwrap();
    write!(output, "{}", scenery.to_dot("")?).unwrap();
    println!("Success");
    print!("\nAnalyzing...");
    scenery.analyze(&opossum_args.analyzer)?;
    println!("Success\n");
    let mut report_path = opossum_args.report_directory.clone();
    report_path.push("report.json");
    print!("Write detector report to {}...", report_path.display());
    let mut output = File::create(report_path).unwrap();
    write!(
        output,
        "{}",
        serde_json::to_string_pretty(&scenery.report(&opossum_args.report_directory)).unwrap()
    )
    .unwrap();
    println!("Success");
    Ok(())
}
