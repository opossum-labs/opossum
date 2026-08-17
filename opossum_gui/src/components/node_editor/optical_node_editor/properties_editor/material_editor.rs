use crate::components::node_editor::{
    hooks::use_synced_signal,
    inputs::input_components::LabeledSelect,
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::{
        on_save_proptype_handler, refractive_index_editor::RefractiveIndexEditor,
    },
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{material::Material, refractive_index::RefractiveIndexType};
use uuid::Uuid;

/// Label of the only way to define a material that this GUI can offer today.
const DEFINED_BY_REFRACTIVE_INDEX: &str = "Manual (refractive index)";

/// Editor for a node's `material` property: a selector for *how* the material is defined plus the
/// editor belonging to the chosen way.
///
/// The dropdown-plus-parameters composition is the one `RefractiveIndexEditor` uses. It is worth
/// having here even while there is a single way to choose: the selector is the place a named
/// substance from the material registry appears once the GUI can reach it, so the panel does not
/// have to be restructured for that - a second entry simply shows up in the list and brings its own
/// editor.
///
/// # Arguments
///
/// * `node_id` - id of the node whose property is edited.
/// * `material` - the material to show.
/// * `property_key` - name of the edited property, needed for the change event.
/// * `on_change` - handler that carries a property change towards the backend.
/// * `readonly` - whether the inputs are shown read-only.
#[component]
pub fn MaterialEditor(
    node_id: Memo<Uuid>,
    material: Material,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let material_sig = use_synced_signal(material);

    // `Material` converts into `Proptype::Material(AssetRef::Inline(..))`, so editing a material
    // always yields an embedded one - which is what a hand-defined material is.
    let on_save = on_save_proptype_handler(
        material_sig,
        property_key.clone(),
        on_change,
        node_id.into(),
    );

    // Replace only the index model inside the current material, so its identity (uuid, name) and
    // every other datum survive an index edit.
    let on_save_refractive_index = EventHandler::new(move |ref_ind_type: RefractiveIndexType| {
        let mut material = material_sig.peek().clone();
        material.optical.refractive_index = ref_ind_type;
        on_save.call(material);
    });

    let ref_ind_type = material_sig.read().refractive_index().clone();
    rsx! {
        LabeledSelect {
            id: format!("materialProperty{property_key}").to_camel_case(),
            label: "Material",
            // Built by hand rather than from an enum iterator: `Material` is a struct, and the
            // choices are ways of *obtaining* one, not variants of it.
            options: vec![(true, DEFINED_BY_REFRACTIVE_INDEX.to_string())],
            readonly,
            onchange: move |_e: Event<FormData>| {},
        }
        div { class: "accordion-content-wrapper-div border-start",
            RefractiveIndexEditor {
                id: format!("refractiveIndexProperty{property_key}"),
                ref_ind_type,
                on_save: on_save_refractive_index,
                readonly,
            }
        }
    }
}
