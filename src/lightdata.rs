use std::fmt::Display;

#[derive(Debug, PartialEq, Clone)]
pub enum LightData {
    Energy(LightDataEnergy),
    Geometric(LightDataGeometric),
    Fourier,
}

impl Display for LightData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LightData::Energy(e) => write!(f, "Energy: {}", e.energy),
            _ => write!(f, "No display defined for this type of LightData"),
        }
    }
}
#[derive(Debug, PartialEq, Clone)]
pub struct LightDataEnergy {
    pub energy: f64,
}

#[derive(Debug, PartialEq, Clone)]
pub struct LightDataGeometric {
    ray: i32,
}
