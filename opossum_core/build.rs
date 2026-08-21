#![allow(missing_docs)]
use std::error::Error;
use vergen_gix::{Emitter, Gix};

pub fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo::rustc-env=OPM_FILE_VERSION=0");
    let gix = Gix::all_git();
    Emitter::default().add_instructions(&gix)?.emit()?;
    Ok(())
}
