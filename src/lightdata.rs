#[derive(Debug, PartialEq, Clone)]
pub enum LightData {
    Energy(LightDataEnergy),
    Geometric(LightDataGeometric),
    Fourier,
}

#[derive(Debug, PartialEq, Clone)]
pub struct LightDataEnergy {
    pub energy: f32,
}

#[derive(Debug, PartialEq, Clone)]
pub struct LightDataGeometric {
    ray: i32,
}
