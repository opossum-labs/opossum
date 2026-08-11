//! Free-function support code shared across the `opossum_backend` HTTP handlers - graph lookups,
//! connection/port-map bookkeeping around node relocation, and a couple of handler-plumbing/HTTP
//! utilities that don't belong to any one resource module.
mod connection_classification;
mod connection_preservation;
mod content_negotiation;
mod graph_lookup;
mod handler_support;
mod port_map_cascade;
mod relocation;

pub use connection_classification::{build_connect_info, reconnect_all, split_sort_connections};
// Never named directly, but `RelocationOutcome::preserved`'s type still needs a crate-wide-reachable
// path for callers outside `helper_functions` to field-access through it.
#[allow(unused_imports)]
pub use connection_preservation::PreservedConnections;
pub use content_negotiation::{Ron, ron_or_json_response};
// `CollectedNode` is never named directly - callers field-access `collect_nodes_recursive`'s
// results - but it still needs a crate-wide-reachable path, same as `PreservedConnections` above.
#[allow(unused_imports)]
pub use graph_lookup::CollectedNode;
pub use graph_lookup::{
    capture_node_connections, check_reference_target_not_nested, collect_group_connections,
    collect_node_refs_and_pos, collect_nodes_recursive, create_new_group_node_info,
    is_reference_target, lowest_common_ancestor_group, map_port, parent_group_id_or_self,
    resolve_reference_chain, validate_relocated_references,
};
pub use handler_support::{analyzer_mut_or_404, apply_and_push_undo};
pub use port_map_cascade::{
    PortMapCascadeRemoval, RemovedPortMapLevel, disconnect_exposed_port_cascades_for_node,
    remove_port_map_cascade, split_cascades_for_response,
};
pub use relocation::{
    relocate_nodes_in_document, relocate_nodes_severing_external_links, remove_relocated_nodes,
    sever_external_links,
};
