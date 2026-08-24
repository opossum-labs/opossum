//! `apply` bodies for the pump-scenario-editing [`Command`] variants: [`Command::AddPumpScenario`],
//! [`Command::RemovePumpScenario`], [`Command::PatchPumpScenario`],
//! [`Command::PatchAnalyzerPumpScenarios`].
use opossum_core::{
    gain::PumpScenario, opm_document::OpmDocument, types::api_types::PumpScenarioItemDto,
};
use uuid::Uuid;

use super::Command;
use crate::{
    error::BackEndErrorResponse,
    helper_functions::{analyzer_mut_or_404, pump_scenario_mut_or_404},
};

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

/// Sets which [`PumpScenario`]s one analyzer runs in.
///
/// A patch of its own rather than folded into [`super::PatchAnalyzer`]: the selection is a
/// document-wide reference list next to the analyzer's own config, not part of what the config
/// means, and deleting a scenario needs to touch exactly this - the analyzer's type is unaffected.
#[derive(Clone)]
pub struct PatchAnalyzerPumpScenarios {
    pub id: Uuid,
    pub old: Vec<Uuid>,
    pub new: Vec<Uuid>,
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
    let scenario = pump_scenario_mut_or_404(document, id)?;
    *scenario = new.clone();
    Ok(Command::PatchPumpScenario(PatchPumpScenario {
        id,
        old: new,
        new: old,
    }))
}

/// Re-inserts a previously removed pump scenario under its original id, returning the
/// [`Command::RemovePumpScenario`] that undoes it.
pub(super) fn apply_add_pump_scenario(
    document: &mut OpmDocument,
    item: PumpScenarioItemDto,
) -> Command {
    document.insert_pump_scenario(item.id, item.scenario.clone());
    Command::RemovePumpScenario(item)
}

/// Removes the pump scenario with the given id, returning the [`Command::AddPumpScenario`] that
/// undoes it.
///
/// This alone does **not** restore any analyzer that had selected the scenario - removing it here
/// (as `OpmDocument::remove_pump_scenario` does unconditionally) strips it from every analyzer's
/// selection as a side effect. Handlers that delete a scenario a user might have selected build a
/// [`Command::Batch`] of this plus one [`Command::PatchAnalyzerPumpScenarios`] per affected analyzer
/// - see `delete_pump_scenario` in `opossum_backend::pump_scenarios`.
///
/// # Errors
///
/// Returns an error if `item.id` doesn't resolve to a pump scenario.
pub(super) fn apply_remove_pump_scenario(
    document: &mut OpmDocument,
    item: PumpScenarioItemDto,
) -> Result<Command, BackEndErrorResponse> {
    document
        .remove_pump_scenario(item.id)
        .ok_or_else(BackEndErrorResponse::pump_scenario_not_found)?;
    Ok(Command::AddPumpScenario(item))
}

/// Replaces an analyzer's pump-scenario selection, returning the
/// [`Command::PatchAnalyzerPumpScenarios`] that undoes it (`old`/`new` swapped).
///
/// # Errors
///
/// Returns an error if `id` doesn't resolve to an analyzer.
pub(super) fn apply_patch_analyzer_pump_scenarios(
    document: &mut OpmDocument,
    cmd: PatchAnalyzerPumpScenarios,
) -> Result<Command, BackEndErrorResponse> {
    let PatchAnalyzerPumpScenarios { id, old, new } = cmd;
    let analyzer_info = analyzer_mut_or_404(document, id)?;
    analyzer_info.set_pump_scenarios(new.clone());
    Ok(Command::PatchAnalyzerPumpScenarios(
        PatchAnalyzerPumpScenarios {
            id,
            old: new,
            new: old,
        },
    ))
}
