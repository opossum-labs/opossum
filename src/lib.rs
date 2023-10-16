//! ![OPOSSUM logo][opossum_logo]
//!
//! This is the documentation for the **OPOSSUM** software package. **OPOSSUM** stands for
//! **Op**en-source **O**ptics **S**imulation **S**oftware and **U**nified **M**odeller.
//!
#![cfg_attr(feature = "doc-images",
cfg_attr(all(),
doc = ::embed_doc_image::embed_image!("opossum_logo", "logo/Logo_text.svg")))]
#![cfg_attr(
    not(feature = "doc-images"),
    doc = "**Doc images not enabled**. Compile with feature `doc-images` and Rust version >= 1.54 \
           to enable."
)]

pub mod analyzer;
pub mod dottable;
pub mod error;
mod light;
pub mod lightdata;
pub mod nodes;
pub mod optic_ports;
/// The basic structure containing the entire optical model
mod optic_scenery;
/// The basic structure representing an optical element
pub mod optical;
/// Handling of node properties
pub mod properties;
pub mod spectrum;
use chrono::DateTime;
pub use optic_scenery::OpticScenery;
/// Module for handling the OPOSSUM CLI
pub mod console;
mod optic_graph;

/// Return the version information of the currently built OPOSSUM executable.
///
/// This function returs a `String` which contains the current Git tag/hash combination as well as
/// the timestamp of this commit.
pub fn get_version() -> String {
    let timestamp = DateTime::parse_from_rfc3339(env!("VERGEN_GIT_COMMIT_TIMESTAMP")).unwrap();
    format!(
        "{} ({})",
        env!("VERGEN_GIT_DESCRIBE"),
        timestamp.format("%Y/%m/%d %H:%M")
    )
}
