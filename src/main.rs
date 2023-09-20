use opossum::{console::{Args, PartialArgs}, error::OpossumError};
use clap::Parser;
type Result<T> = std::result::Result<T, OpossumError>;



fn main() -> Result<()>{
    let test = Args::try_from(PartialArgs::parse())?;

    println!("file path: {}", test.file_path);
    println!("analyzer: {}", test.analyzer);

    Ok(())

}
