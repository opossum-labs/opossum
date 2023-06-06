#[derive(Debug, Clone)]
pub enum OpossumError {
  OpticScenery(String),
  OpticGroup(String),
  Other(String)
}