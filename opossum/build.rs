#![allow(missing_docs)]
use std::error::Error;
use vergen::EmitBuilder;

pub fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    EmitBuilder::builder().all_git().emit()?;
    Ok(())
}
