use opossum_core::types::api_types::ConnectInfo;
use std::ops::Deref;

/// GUI wrapper for [`ConnectInfo`] carrying a persistent, monotonic edge index for E2E testing (playwright).
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeElement {
    info: ConnectInfo,
    edge_index: usize,
}

impl EdgeElement {
    /// Creates a new [`EdgeElement`] with the given connection info and unique index.
    pub const fn new(info: ConnectInfo, edge_index: usize) -> Self {
        Self { info, edge_index }
    }

    /// Returns the monotonic session edge index.
    pub const fn edge_index(&self) -> usize {
        self.edge_index
    }

    /// Returns a reference to the inner [`ConnectInfo`] struct.
    pub const fn info(&self) -> &ConnectInfo {
        &self.info
    }
}

/// Allows transparent access to inner [`ConnectInfo`] fields and methods.
impl Deref for EdgeElement {
    type Target = ConnectInfo;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}
