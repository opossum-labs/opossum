//! The sidebar view of what belongs to the setup as a whole rather than to one node.
use crate::{
    SCENE_REVISION, api,
    components::{
        inputs::material_selector::MaterialSelector, node_editor::accordion::StaticSection,
        scenery_editor::GraphsWorkspaceAction,
    },
};
use dioxus::prelude::*;
use opossum_core::material::Material;

/// Edit the medium the setup is aligned in.
///
/// The model is placed in this medium, whichever analysis examines it; an analysis may still trace
/// its own light through another one. The medium is fetched again on every change of the model -
/// `SCENE_REVISION` covers edits as well as undo and redo - and a new one is sent as a workspace
/// action, which records it for undo and marks the document unsaved.
#[component]
pub fn SetupEditor() -> Element {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let medium = use_resource(move || async move {
        SCENE_REVISION();
        api::get_ambient_material().await
    });
    let on_change = move |material: Material| {
        workspace_processor.send(GraphsWorkspaceAction::SetAmbientMaterial(material));
    };
    rsx! {
        StaticSection { header: "Setup",
            match &*medium.read() {
                Some(Ok(material)) => rsx! {
                    MaterialSelector {
                        label: "Ambient medium".to_string(),
                        material: material.clone(),
                        readonly: false,
                        on_change,
                    }
                    div { class: "text-muted small",
                        "The medium the setup is aligned in: the model is placed in it, whichever analysis examines it."
                    }
                },
                Some(Err(error)) => rsx! {
                    div { class: "text-muted small", "{error}" }
                },
                None => rsx! {},
            }
        }
    }
}
