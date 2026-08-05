//! `apply`/`describe` body for the [`Command::SetViewport`] variant.
//!
//! Unlike every other command, this one **does not mutate the document** - the canvas viewport
//! (pan/zoom) is purely a GUI concern and is never part of the saved `.opm`. `SetViewport` exists only
//! so a viewport change is a reversible entry on the same undo stack as document edits: applying it emits
//! a [`DocumentChange::ViewportChanged`] the GUI reacts to, and its inverse just swaps the two viewports.
use opossum_core::types::api_types::{DocumentChange, Viewport};

use super::Command;

/// A reversible viewport change: applying it moves the camera to `to`; its inverse moves it back to
/// `from`. Both carry the same `graph_id` (a change never crosses tabs).
#[derive(Clone)]
pub struct SetViewport {
    /// The viewport that applying the *inverse* restores.
    pub from: Viewport,
    /// The viewport that applying this command moves the camera to.
    pub to: Viewport,
    /// Whether a subsequent coalescing move may merge into this entry. Set for scroll-zoom ticks, unset
    /// for discrete gestures (pan, center, zoom-to-fit) so gesture types stay separate undo steps.
    pub coalescing: bool,
}

/// Returns the [`Command::SetViewport`] that undoes `cmd` (its `from`/`to` swapped). Does not touch the
/// document - the viewport lives only in the GUI; the effect is carried by [`describe_set_viewport`].
pub(super) const fn apply_set_viewport(cmd: SetViewport) -> Command {
    let SetViewport {
        from,
        to,
        coalescing,
    } = cmd;
    Command::SetViewport(SetViewport {
        from: to,
        to: from,
        coalescing,
    })
}

/// Describes the effect of applying `cmd` in the GUI-facing [`DocumentChange`] shape: move the camera of
/// `cmd.to.graph_id` to `cmd.to`.
pub(super) fn describe_set_viewport(cmd: &SetViewport) -> Vec<DocumentChange> {
    vec![DocumentChange::ViewportChanged {
        graph_id: cmd.to.graph_id,
        zoom: cmd.to.zoom,
        shift: cmd.to.shift,
    }]
}
