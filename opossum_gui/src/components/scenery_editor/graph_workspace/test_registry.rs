use opossum_core::types::api_types::ConnectInfo;
use std::collections::HashMap;
use std::hash::Hash;
use uuid::Uuid;

/// Generic registry that assigns and caches stable sequential test indices for elements
/// identified by a unique key of type `K`. Indices are preserved across deletions and
/// subsequent undo/redo actions.
#[derive(Clone, PartialEq, Debug)]
pub struct IndexRegistry<K: Hash + Eq> {
    indices: HashMap<K, usize>,
    next_index: usize,
}

impl<K: Hash + Eq> Default for IndexRegistry<K> {
    fn default() -> Self {
        Self {
            indices: HashMap::new(),
            next_index: 0,
        }
    }
}

impl<K: Hash + Eq + Clone> IndexRegistry<K> {
    /// Returns the existing sequential test index for `key`, or allocates and caches
    /// the next sequential index if the key is registered for the first time.
    pub fn get_or_create(&mut self, key: K) -> usize {
        if let Some(&index) = self.indices.get(&key) {
            index
        } else {
            let index = self.next_index;
            self.next_index += 1;
            self.indices.insert(key, index);
            index
        }
    }
}

/// Unique identity of an edge connecting two node ports.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct EdgeKey {
    pub src_uuid: Uuid,
    pub src_port: String,
    pub target_uuid: Uuid,
    pub target_port: String,
}

impl From<&ConnectInfo> for EdgeKey {
    fn from(info: &ConnectInfo) -> Self {
        Self {
            src_uuid: info.src_uuid(),
            src_port: info.src_port().to_string(),
            target_uuid: info.target_uuid(),
            target_port: info.target_port().to_string(),
        }
    }
}
