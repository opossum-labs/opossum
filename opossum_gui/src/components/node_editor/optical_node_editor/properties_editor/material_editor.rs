use crate::components::node_editor::{
    hooks::use_synced_signal,
    inputs::{input_components::LabeledSelect, select_options_from_enum_iterator},
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::{
        on_save_proptype_handler, refractive_index_editor::RefractiveIndexEditor,
    },
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{
    material::Material, refractive_index::RefractiveIndexType,
    utils::default_from_name::DefaultFromName,
};
use uuid::Uuid;

/// Editor for a node's `material` property: a selector for *how* the material is defined plus the
/// editor belonging to the chosen way.
///
/// The dropdown-plus-parameters composition is the one `RefractiveIndexEditor` and
/// `GainModelEditor` use. It is worth having here even while `Material` has a single variant: the
/// selector is the place a named substance from a material library appears once one exists, so the
/// user interface does not have to be restructured for it - a new variant simply shows up in the
/// list and brings its own editor.
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

    let on_save = on_save_proptype_handler(
        material_sig,
        property_key.clone(),
        on_change,
        node_id.into(),
    );

    rsx! {
        LabeledSelect {
            id: format!("materialProperty{property_key}").to_camel_case(),
            label: "Material",
            options: select_options_from_enum_iterator(&*material_sig.read(), None),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(material) = Material::default_from_name(val.as_str()) {
                    on_save.call(material);
                }
            },
        }
        div { class: "accordion-content-wrapper-div border-start",
            {material_definition_editor(material_sig.into(), on_save, &property_key, readonly)}
        }
    }
}

/// Return the editor belonging to the selected way of defining the material.
///
/// A variant this GUI does not know yet (`Material` is `#[non_exhaustive]`) shows the selector
/// alone rather than an empty editor.
///
/// # Arguments
///
/// * `material_sig` - the material currently shown.
/// * `on_save` - handler that saves a changed material.
/// * `property_key` - name of the edited property, used to build unique DOM ids.
/// * `readonly` - whether the inputs are shown read-only.
fn material_definition_editor(
    material_sig: ReadSignal<Material>,
    on_save: EventHandler<Material>,
    property_key: &str,
    readonly: bool,
) -> Element {
    match &*material_sig.read() {
        Material::RefractiveIndex(ref_ind_type) => rsx! {
            RefractiveIndexEditor {
                id: format!("refractiveIndexProperty{property_key}"),
                ref_ind_type: ref_ind_type.clone(),
                on_save: move |ref_ind_type: RefractiveIndexType| on_save.call(ref_ind_type.into()),
                readonly,
            }
        },
        _ => rsx! {},
    }
}
