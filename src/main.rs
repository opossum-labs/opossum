use std::fs::{self, File};
use std::io::Write;

use clap::Parser;
use opossum::{
    OpticScenery,
    {
        console::{show_intro, Args, PartialArgs},
        error::OpossumError,
    },
};

type Result<T> = std::result::Result<T, OpossumError>;

fn main() -> Result<()> {
    //not necessary, just for fun
    show_intro();
    if let Err(e) = do_it() {
        println!("Error: {}", e);
        Err(e)
    } else {
        Ok(())
    }
}

fn do_it() -> Result<()> {
    let opossum_args = Args::try_from(PartialArgs::parse())?;

    print!("\nReading model...");
    let contents = fs::read_to_string(&opossum_args.file_path).map_err(|e| {
        OpossumError::Console(format!(
            "cannot read file {} : {}",
            opossum_args.file_path.display(),
            e
        ))
    })?;
    let mut scenery: OpticScenery = serde_yaml::from_str(&contents)
        .map_err(|e| OpossumError::OpticScenery(format!("error while parsing model file: {}", e)))?;
    println!("Success");
    print!("\nAnalyzing...");
    scenery.analyze(&opossum_args.analyzer)?;
    println!("Success\n");
    scenery.report();
    let mut dot_path = opossum_args.report_directory.clone();
    dot_path.push(opossum_args.file_path.file_stem().unwrap());
    dot_path.set_extension("dot");
    print!("Write diagram to {}...", dot_path.display());
    let mut output = File::create(dot_path).unwrap();
    write!(output, "{}", scenery.to_dot("")?).unwrap();
    println!("Success");
    Ok(())
}
