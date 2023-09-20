//! Data structures containing the light information flowing between [`Opticals`](crate::optical::Optical).
use serde_derive::{Serialize, Deserialize};
use std::fmt::Display;
use uom::fmt::DisplayStyle::Abbreviation;
use uom::si::{energy::joule, f64::Energy};

use crate::spectrum::Spectrum;

/// Data structure defining the light properties. The actuals data type used depends on the
/// [`AnalyzerType`](crate::analyzer::AnalyzerType). For example, an energy analysis ([`LightData::Energy`]) only
/// contains a [`Spectrum`] information, while a geometric analysis ([`LightData::Geometric]) constains a set of optical
/// ray data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LightData {
    /// data type used for energy analysis.
    Energy(DataEnergy),
    /// data type used for geometric optics analysis (ray tracing)
    Geometric(DataGeometric),
    /// placeholder value for future Fourier optics analysis, nothing implementd yet.
    Fourier,
}
impl LightData {
    pub fn export(&self, file_name: &str) {
        match self {
            LightData::Energy(d) => {
                d.spectrum.to_plot(file_name);
            }
            _ => println!("no export function defined for this type of LightData"),
        }
    }
}
impl Display for LightData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LightData::Energy(e) => {
                let ef = Energy::format_args(joule, Abbreviation);
                write!(
                    f,
                    "Energy: {}",
                    ef.with(Energy::new::<joule>(e.spectrum.total_energy()))
                )
            }
            _ => write!(f, "No display defined for this type of LightData"),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataEnergy {
    pub spectrum: Spectrum,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataGeometric {
    _ray: i32,
}
