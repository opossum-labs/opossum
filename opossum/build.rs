#![allow(missing_docs)]
use std::error::Error;
use vergen_git2::{Emitter, Git2Builder};

pub fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo::rustc-env=OPM_FILE_VERSION=0");
    let git2 = Git2Builder::all_git()?;
    Emitter::default().add_instructions(&git2)?.emit()?;
    Ok(())
}
