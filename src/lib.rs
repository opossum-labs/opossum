//! This is the documentation for the **OPOSSUM** software package. **OPOSSUM** stands for
//! **Op**en-source **O**ptics **S**imulation **S**oftware and **U**nified **M**odeller.
//!
mod light;
pub mod lightdata;
/// The basic structure representing an optical element
pub mod optical;

pub mod dottable;

/// The basic structure containing the entire optical model
mod optic_scenery;

pub mod optic_ports;

pub mod nodes;

pub mod analyzer;

pub mod error;

pub mod spectrum;

pub mod properties;

use chrono::DateTime;
pub use optic_scenery::OpticScenery;

pub mod console;

mod optic_graph;

fn get_version() -> String {
    let timestamp = DateTime::parse_from_rfc3339(env!("VERGEN_GIT_COMMIT_TIMESTAMP")).unwrap();
    format!(
        "{} ({})",
        env!("VERGEN_GIT_DESCRIBE"),
        timestamp.format("%Y/%m/%d %H:%M")
    )
}
