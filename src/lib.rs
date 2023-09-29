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

pub use optic_scenery::OpticScenery;

pub mod console;
