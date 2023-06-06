#[derive(Debug, Clone)]
pub enum OpossumError {
  OpticScenery(String),
  OpticGroup(String),
  OpticPort(String),
  Other(String)
}