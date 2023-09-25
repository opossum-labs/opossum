use opossum::{{
    console::{Args, PartialArgs, show_intro}, 
    error::OpossumError}
};
use clap::Parser;
type Result<T> = std::result::Result<T, OpossumError>;



fn main() -> Result<()>{    
    //not necessary, just for fun
    show_intro();
    let opossum_args = Args::try_from(PartialArgs::parse())?;

    println!("file path: {}", opossum_args.file_path);
    println!("analyzer: {}", opossum_args.analyzer);
    println!("report directory: {}", opossum_args.report_directory);

    //todo: 
    //-create optic scenery from yaml
    //-create analzyer for scenery
    //-create report

    Ok(())

}
