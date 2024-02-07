//! ![OPOSSUM logo][opossum_logo]
//!
//! This is the documentation for the **OPOSSUM** software package. **OPOSSUM** stands for
//! **Op**en-source **O**ptics **S**imulation **S**oftware and **U**nified **M**odeler.
//!
#![cfg_attr(feature = "doc-images",
cfg_attr(all(),
doc = ::embed_doc_image::embed_image!("opossum_logo", "logo/Logo_text.svg")))]
#![cfg_attr(
    not(feature = "doc-images"),
    doc = "**Doc images not enabled**. Compile with feature `doc-images` and Rust version >= 1.54 \
           to enable."
)]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::module_name_repetitions))]

pub mod analyzer;
pub mod aperture;
pub mod dottable;
pub mod error;
mod light;
pub mod lightdata;
pub mod nodes;
pub mod optic_ports;
mod optic_scenery;
pub mod optical;
pub mod plottable;
pub mod properties;
pub mod spectrum;
pub mod spectrum_helper;
use chrono::DateTime;
pub use optic_scenery::OpticScenery;
pub mod console;
mod optic_graph;
pub mod optic_ref;
pub mod ray;
pub mod rays;
pub use ray::SplittingConfig;
pub mod reporter;
pub mod surface;
//use surface::Plane;
//use surface::Surface;

/// Return the version information of the currently built OPOSSUM executable.
///
/// This function returs a `String` which contains the current Git tag/hash combination as well as
/// the timestamp of this commit.
#[must_use]
pub fn get_version() -> String {
    let timestamp = DateTime::parse_from_rfc3339(env!("VERGEN_GIT_COMMIT_TIMESTAMP")).map_or_else(
        |_| String::from("invalid timestamp"),
        |timestamp| timestamp.format("%Y/%m/%d %H:%M").to_string(),
    );
    format!("{} ({})", env!("VERGEN_GIT_DESCRIBE"), timestamp)
}
#[cfg(test)]
mod test {
    use super::*;
    use regex::Regex;
    #[test]
    fn get_ver() {
        let version_string = get_version();
        let re = Regex::new(r"(.*) \(\d{4}/\d{2}/\d{2} \d{2}:\d{2}\)").unwrap();
        assert!(re.is_match(&version_string));
    }
}
