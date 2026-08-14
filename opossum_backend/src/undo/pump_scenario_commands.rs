//! `apply` body for the pump-scenario-editing [`Command`] variant: [`Command::PatchPumpScenario`].
use opossum_core::{gain::PumpScenario, opm_document::OpmDocument};
use uuid::Uuid;

use super::Command;
use crate::error::BackEndErrorResponse;

/// Replaces a [`PumpScenario`] as a whole.
///
/// One command covers every edit to an operating point - renaming it, adding a node to it, changing
/// a node's gain model, dropping the entries of deleted nodes - because a scenario is small and
/// replacing it wholesale is what makes each of those reversible without a variant of its own.
#[derive(Clone)]
pub struct PatchPumpScenario {
    pub id: Uuid,
    pub old: PumpScenario,
    pub new: PumpScenario,
}

/// Replaces a pump scenario, returning the [`Command::PatchPumpScenario`] that undoes it (`old`/`new`
/// swapped).
///
/// # Errors
///
/// Returns an error if `id` doesn't resolve to a pump scenario.
pub(super) fn apply_patch_pump_scenario(
    document: &mut OpmDocument,
    cmd: PatchPumpScenario,
) -> Result<Command, BackEndErrorResponse> {
    let PatchPumpScenario { id, old, new } = cmd;
    let scenario = document
        .pump_scenario_mut(id)
        .ok_or_else(BackEndErrorResponse::pump_scenario_not_found)?;
    *scenario = new.clone();
    Ok(Command::PatchPumpScenario(PatchPumpScenario {
        id,
        old: new,
        new: old,
    }))
}
