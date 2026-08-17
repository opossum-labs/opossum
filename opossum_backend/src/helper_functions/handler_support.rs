use actix_web::HttpResponse;
use opossum_core::{
    gain::PumpScenario,
    opm_document::{AnalyzerInfo, OpmDocument},
};
use parking_lot::MutexGuard;
use uuid::Uuid;

use crate::{app_state::AppState, error::BackEndErrorResponse, undo::Command};

/// Looks up the analyzer with the given `id` in `document`, mutably.
///
/// # Errors
///
/// Returns a 404 ([`BackEndErrorResponse::analyzer_not_found`]) if `id` doesn't resolve to an
/// analyzer.
pub fn analyzer_mut_or_404(
    document: &mut OpmDocument,
    id: Uuid,
) -> Result<&mut AnalyzerInfo, BackEndErrorResponse> {
    document
        .analyzer_mut(id)
        .ok_or_else(BackEndErrorResponse::analyzer_not_found)
}

/// Looks up the pump scenario with the given `id` in `document`, mutably.
///
/// # Errors
///
/// Returns a 404 ([`BackEndErrorResponse::pump_scenario_not_found`]) if `id` doesn't resolve to a
/// pump scenario.
pub fn pump_scenario_mut_or_404(
    document: &mut OpmDocument,
    id: Uuid,
) -> Result<&mut PumpScenario, BackEndErrorResponse> {
    document
        .pump_scenario_mut(id)
        .ok_or_else(BackEndErrorResponse::pump_scenario_not_found)
}

/// Applies `command` to `document`, pushes its inverse onto the undo stack (unless `record_undo` is
/// `false` - used only for edits that are deliberately excluded from history, e.g. patching the
/// scenery root), then drops `document`'s lock and returns the standard `204 No Content` response
/// shared by every field-patch endpoint.
///
/// # Errors
///
/// Returns an error if `command.apply` fails.
pub fn apply_and_push_undo(
    data: &AppState,
    mut document: MutexGuard<'_, OpmDocument>,
    command: Command,
    record_undo: bool,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let inverse = command.apply(&mut document)?;
    if record_undo {
        data.push_undo(inverse);
    }
    drop(document);
    Ok(HttpResponse::NoContent().finish())
}
