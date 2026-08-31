#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendStatus {
    #[default]
    Checking,
    Connected,
    Disconnected,
}

impl BackendStatus {
    pub const fn is_connected(self) -> bool {
        matches!(self, Self::Connected)
    }
}
