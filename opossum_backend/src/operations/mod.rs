//! Cross-cutting "macro" node operations: copy/paste, cut, group conversion, and moving nodes
//! between groups. Each is its own submodule; shared plumbing (used by more than one of them)
//! lives here, mirroring `crate::undo`'s split.
mod convert_to_group;
mod copy;
mod cut;
mod move_nodes;
mod paste;

use nalgebra::Point2;
use opossum_core::{core_optics::NodeAttrExt, utils::LockExt};
use utoipa_actix_web::service_config::ServiceConfig;

// Not named anywhere in this crate's non-test code, but `document.rs`/`nodes/core.rs` test modules
// build a minimal test `App` via the fully-qualified `crate::operations::post_convert_nodes_to_group`
// path, which resolves only through this re-export (`convert_to_group` itself stays a private module).
#[allow(unused_imports)]
pub use convert_to_group::post_convert_nodes_to_group;

use crate::{app_state::NodeCacheItem, error::BackEndErrorResponse};

/// The top-left corner of the given cached nodes' current GUI positions - the anchor a paste/cut
/// shifts the pasted-in copies relative to. Shared by [`paste::post_paste_nodes`] and
/// [`cut::post_cut_nodes`].
fn upper_left_corner_of_nodes(
    nodes: &[NodeCacheItem],
) -> Result<Point2<f64>, BackEndErrorResponse> {
    let mut corner = Point2::new(f64::INFINITY, f64::INFINITY);

    for node in nodes {
        let pos = match node {
            NodeCacheItem::Optical(optical_node) => {
                let node = optical_node.optical_ref.lock_opm()?;
                node.gui_position().unwrap_or_else(Point2::origin)
            }
            NodeCacheItem::Analyzer(analyzer_dto) => {
                // Access info from DTO
                analyzer_dto
                    .info
                    .gui_position()
                    .unwrap_or_else(Point2::origin)
            }
        };

        corner.x = corner.x.min(pos.x);
        corner.y = corner.y.min(pos.y);
    }

    Ok(corner)
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(copy::post_copy_nodes);
    cfg.service(paste::post_paste_nodes);
    cfg.service(cut::post_cut_nodes);

    cfg.service(convert_to_group::post_convert_nodes_to_group);
    cfg.service(move_nodes::post_move_nodes);
}
