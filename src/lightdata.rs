use std::fmt::Display;
use uom::fmt::DisplayStyle::Abbreviation;
use uom::si::{energy::joule, f64::Energy};

use crate::spectrum::Spectrum;

#[derive(Debug, Clone)]
pub enum LightData {
    Energy(DataEnergy),
    Geometric(DataGeometric),
    Fourier,
}
impl LightData {
    pub fn export(&self, file_name: &str) {
        match self {
            LightData::Energy(d) => {
                d.spectrum.to_plot(file_name);
            },
           _ => println!("no export function defined for this type of LightData")
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
#[derive(Debug, Clone)]
pub struct DataEnergy {
    pub spectrum: Spectrum,
}

#[derive(Debug, Clone)]
pub struct DataGeometric {
    _ray: i32,
}
