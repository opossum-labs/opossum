//! `apply` body for the document-level [`Command::PatchAmplifierNodes`] variant.
use std::collections::HashSet;

use opossum_core::opm_document::OpmDocument;
use uuid::Uuid;

use super::Command;

/// Replaces the whole amplifier-candidate set (`OpmDocument::amplifier_nodes`) at once.
///
/// One command covers every edit to the set - marking or unmarking a node, or pruning a deleted
/// node's entry - because the set is small and replacing it wholesale is what makes each of those
/// reversible without a variant of its own, the same convention [`super::PatchPumpScenario`] uses
/// for one scenario.
#[derive(Clone)]
pub struct PatchAmplifierNodes {
    /// The candidate set in place beforehand, so `apply` can build the inverse that restores it.
    pub old: HashSet<Uuid>,
    /// The candidate set to install.
    pub new: HashSet<Uuid>,
}

/// Installs `cmd.new` as the document's amplifier-candidate set, returning the
/// [`Command::PatchAmplifierNodes`] that undoes it (`old`/`new` swapped).
///
/// This replaces the set wholesale via [`OpmDocument::set_amplifier_nodes`], which - unlike
/// [`OpmDocument::set_is_amplifier_node`] - does not touch any [`opossum_core::gain::PumpScenario`]
/// as a side effect: a handler that unmarks a candidate configured in one or more scenarios has to
/// fold one [`Command::PatchPumpScenario`] per affected scenario into the same undo batch (see
/// `put_node_is_amplifier` in `opossum_backend::nodes::amplifier_candidates`), the same way
/// `delete_pump_scenario` already does for analyzer selections.
///
/// # Arguments
///
/// - `document`: the live document whose candidate set is replaced.
/// - `cmd`: the old/new candidate sets.
///
/// # Returns
///
/// The inverse [`Command::PatchAmplifierNodes`] that restores the previous set.
pub(super) fn apply_patch_amplifier_nodes(
    document: &mut OpmDocument,
    cmd: PatchAmplifierNodes,
) -> Command {
    let PatchAmplifierNodes { old, new } = cmd;
    document.set_amplifier_nodes(new.clone());
    Command::PatchAmplifierNodes(PatchAmplifierNodes { old: new, new: old })
}
