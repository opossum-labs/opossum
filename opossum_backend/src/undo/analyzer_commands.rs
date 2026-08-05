//! `apply` bodies for the analyzer-editing [`Command`] variants: [`Command::AddAnalyzer`],
//! [`Command::RemoveAnalyzer`], [`Command::PatchAnalyzer`], [`Command::RepositionAnalyzer`].
use nalgebra::Point2;
use opossum_core::{
    opm_document::OpmDocument, prelude::AnalyzerType, types::api_types::AnalyzerItemDto,
};
use uuid::Uuid;

use super::Command;
use crate::{error::BackEndErrorResponse, helper_functions::analyzer_mut_or_404};

/// Replaces an analyzer's config.
#[derive(Clone)]
pub struct PatchAnalyzer {
    pub id: Uuid,
    pub old: AnalyzerType,
    pub new: AnalyzerType,
}

/// Repositions an analyzer on the GUI canvas.
#[derive(Clone, Copy)]
pub struct RepositionAnalyzer {
    pub id: Uuid,
    pub old_pos: (f64, f64),
    pub new_pos: (f64, f64),
}

/// Re-inserts a previously removed analyzer under its original id, returning the
/// [`Command::RemoveAnalyzer`] that undoes it.
pub(super) fn apply_add_analyzer(document: &mut OpmDocument, analyzer: AnalyzerItemDto) -> Command {
    document.insert_analyzer(analyzer.id, analyzer.info.clone());
    Command::RemoveAnalyzer(analyzer)
}

/// Removes the analyzer with the given id, returning the [`Command::AddAnalyzer`] that undoes it.
///
/// # Errors
///
/// Returns an error if `id` doesn't resolve to an analyzer.
pub(super) fn apply_remove_analyzer(
    document: &mut OpmDocument,
    analyzer: AnalyzerItemDto,
) -> Result<Command, BackEndErrorResponse> {
    document.remove_analyzer(analyzer.id)?;
    Ok(Command::AddAnalyzer(analyzer))
}

/// Replaces an analyzer's config, returning the [`Command::PatchAnalyzer`] that undoes it (`old`/`new`
/// swapped).
///
/// # Errors
///
/// Returns an error if `id` doesn't resolve to an analyzer.
pub(super) fn apply_patch_analyzer(
    document: &mut OpmDocument,
    cmd: PatchAnalyzer,
) -> Result<Command, BackEndErrorResponse> {
    let PatchAnalyzer { id, old, new } = cmd;
    let analyzer_info = analyzer_mut_or_404(document, id)?;
    analyzer_info.set_analyzer_type(&new);
    Ok(Command::PatchAnalyzer(PatchAnalyzer {
        id,
        old: new,
        new: old,
    }))
}

/// Repositions an analyzer on the GUI canvas, returning the [`Command::RepositionAnalyzer`] that undoes
/// it (`old_pos`/`new_pos` swapped).
///
/// # Errors
///
/// Returns an error if `id` doesn't resolve to an analyzer.
pub(super) fn apply_reposition_analyzer(
    document: &mut OpmDocument,
    cmd: RepositionAnalyzer,
) -> Result<Command, BackEndErrorResponse> {
    let RepositionAnalyzer {
        id,
        old_pos,
        new_pos,
    } = cmd;
    let analyzer_info = analyzer_mut_or_404(document, id)?;
    analyzer_info.set_gui_position(Some(Point2::new(new_pos.0, new_pos.1)));
    Ok(Command::RepositionAnalyzer(RepositionAnalyzer {
        id,
        old_pos: new_pos,
        new_pos: old_pos,
    }))
}
