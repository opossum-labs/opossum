use super::edges_component::EdgePort;

#[derive(Clone, PartialEq)]
pub struct Edge {
    src_port: EdgePort,
    target_port: EdgePort,
    distance: f64,
}
impl Edge {
    #[must_use]
    pub const fn new(src_port: EdgePort, target_port: EdgePort, distance: f64) -> Self {
        Self {
            src_port,
            target_port,
            distance,
        }
    }
    pub fn src_port(&self) -> &EdgePort {
        &self.src_port
    }
    pub fn target_port(&self) -> &EdgePort {
        &self.target_port
    }
    pub fn distance(&self) -> f64 {
        self.distance
    }
}
