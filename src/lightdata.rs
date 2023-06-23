#[derive(Debug, PartialEq)]
pub enum LightData {
  Energy(LightDataEnergy),
  Geometric(LightDataGeometric),
  Fourier
}

#[derive(Debug, PartialEq)]
pub struct LightDataEnergy {
  energy: f32
}

#[derive(Debug, PartialEq)]
pub struct LightDataGeometric {
  ray: i32
}