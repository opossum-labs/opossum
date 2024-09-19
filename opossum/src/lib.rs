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
#![allow(clippy::module_name_repetitions)]

#[cfg(feature = "bevy")]
pub mod bevy_main;
#[cfg(feature = "bevy")]
pub mod bevy_scene;
#[cfg(feature = "bevy")]
mod scenery_bevy_data;

pub mod analyzable;
pub mod analyzers;
pub mod aperture;
pub mod console;
pub mod dottable;
pub mod energy_distributions;
pub mod error;
mod light_flow;
mod light_result;
pub mod lightdata;
pub mod nodes;
pub mod opm_document;
pub mod optic_node;
mod optic_port;
pub mod optic_ports;
pub mod optic_ref;
mod optic_senery_rsc;
pub mod plottable;
mod port_map;
pub mod position_distributions;
pub mod properties;
pub mod ray;
pub mod rays;
pub mod refractive_index;
pub mod spectral_distribution;
// pub mod render;
pub mod coatings;
pub mod reporter;
pub mod spectrum;
pub mod spectrum_helper;
pub mod surface;
pub mod utils;

pub use opm_document::OpmDocument;
pub use optic_senery_rsc::SceneryResources;

#[cfg(feature = "bevy")]
pub use scenery_bevy_data::SceneryBevyData;

use chrono::DateTime;
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
