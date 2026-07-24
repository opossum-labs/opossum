//! `apply`/`describe` body for the document-level [`Command::PatchGlobalConf`] variant.
use opossum_core::{
    core_optics::SceneryResources, opm_document::OpmDocument, types::api_types::DocumentChange,
};

use super::Command;

/// Replaces the document's global [`SceneryResources`] config. `old` is the value in place beforehand,
/// so `apply` can build the inverse that restores it.
#[derive(Clone)]
pub struct PatchGlobalConf {
    /// The config to restore on undo.
    pub old: SceneryResources,
    /// The config to apply.
    pub new: SceneryResources,
}

/// Applies `cmd.new` as the document's global config, returning the [`Command::PatchGlobalConf`] that
/// undoes it (`old`/`new` swapped).
///
/// # Arguments
///
/// - `document`: the live document whose global config is replaced.
/// - `cmd`: the old/new configs.
///
/// # Returns
///
/// The inverse [`Command::PatchGlobalConf`] that restores the previous config.
pub(super) fn apply_patch_global_conf(document: &mut OpmDocument, cmd: PatchGlobalConf) -> Command {
    let PatchGlobalConf { old, new } = cmd;
    document.set_global_conf(new.clone());
    Command::PatchGlobalConf(PatchGlobalConf { old: new, new: old })
}

/// Describes the effect of a [`Command::PatchGlobalConf`] in the GUI-facing [`DocumentChange`] shape.
///
/// The global config has no canvas element the GUI mirrors incrementally, so this reports no
/// `DocumentChange` for now - the document is still correctly reverted by `apply`, and a future
/// global-config editor can add a matching change variant when there's something to refresh.
pub(super) const fn describe_patch_global_conf() -> Vec<DocumentChange> {
    Vec::new()
}
