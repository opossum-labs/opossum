//! `apply` body for the document-level [`Command::PatchAmbientMaterial`] variant.
use opossum_core::{material::Material, opm_document::OpmDocument};

use super::Command;

/// Replaces the medium the setup is aligned in (`OpmDocument::ambient_material`).
#[derive(Clone)]
pub struct PatchAmbientMaterial {
    /// The medium in place beforehand, so `apply` can build the inverse that restores it.
    pub old: Material,
    /// The medium to install.
    pub new: Material,
}

/// Installs `cmd.new` as the medium the setup is aligned in, returning the
/// [`Command::PatchAmbientMaterial`] that undoes it (`old`/`new` swapped).
///
/// # Arguments
///
/// - `document`: the live document whose medium is replaced.
/// - `cmd`: the old/new media.
///
/// # Returns
///
/// The inverse [`Command::PatchAmbientMaterial`] that restores the previous medium.
pub(super) fn apply_patch_ambient_material(
    document: &mut OpmDocument,
    cmd: PatchAmbientMaterial,
) -> Command {
    let PatchAmbientMaterial { old, new } = cmd;
    document.set_ambient_material(new.clone());
    Command::PatchAmbientMaterial(Box::new(PatchAmbientMaterial { old: new, new: old }))
}
