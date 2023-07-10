use std::fmt::Display;
use uom::si::{f64::Energy, energy::joule};
use uom::fmt::DisplayStyle::Abbreviation;

#[derive(Debug, PartialEq, Clone)]
pub enum LightData {
    Energy(LightDataEnergy),
    Geometric(LightDataGeometric),
    Fourier,
}

impl Display for LightData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            
            LightData::Energy(e) => {
                let ef = Energy::format_args(joule, Abbreviation);
                write!(f, "Energy: {}", ef.with(e.energy))
            } ,
            _ => write!(f, "No display defined for this type of LightData"),
        }
    }
}
#[derive(Debug, PartialEq, Clone)]
pub struct LightDataEnergy {
    pub energy: Energy,
}

#[derive(Debug, PartialEq, Clone)]
pub struct LightDataGeometric {
    ray: i32,
}
