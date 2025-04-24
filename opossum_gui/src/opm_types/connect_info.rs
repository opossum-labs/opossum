use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Connection Information
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectInfo {
    /// UUID of the source node
    src_uuid: Uuid,
    /// name of the (outgoing) source port
    src_port: String,
    /// UUID of the target node
    target_uuid: Uuid,
    /// name of the (incoming) target port
    target_port: String,
    /// geometric distance between nodes (optical axis) in meters.
    distance: f64,
}
impl ConnectInfo {
    #[must_use]
    pub const fn new(
        src_uuid: Uuid,
        src_port: String,
        target_uuid: Uuid,
        target_port: String,
        distance: f64,
    ) -> Self {
        Self {
            src_uuid,
            src_port,
            target_uuid,
            target_port,
            distance,
        }
    }
    #[must_use]
    pub const fn src_uuid(&self) -> Uuid {
        self.src_uuid
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn src_port(&self) -> &str {
        &self.src_port
    }
    #[must_use]
    pub const fn target_uuid(&self) -> Uuid {
        self.target_uuid
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn target_port(&self) -> &str {
        &self.target_port
    }
    #[must_use]
    pub const fn distance(&self) -> f64 {
        self.distance
    }
}
