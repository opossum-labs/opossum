#![allow(clippy::derive_partial_eq_without_eq)]

mod angle_editor;
mod bool_editor;
mod curvature_editor;
mod f64_editor;
mod filter_type_editor;
mod fluence_estimator_editor;
mod i32_editor;
mod isometry_option_editor;
mod length_editor;
mod length_option_editor;
mod linear_density_editor;
mod material_editor;
mod refractive_index_editor;
mod splitter_type_editor;
mod string_editor;
mod vec2_editor;
mod vec3_editor;

use crate::components::node_editor::{
    accordion::{AccordionItem, content_id_for_panel},
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::properties_editor::{
        angle_editor::AngleEditor, bool_editor::BoolEditor, curvature_editor::CurvatureEditor,
        f64_editor::F64Editor, filter_type_editor::FilterTypeEditor,
        fluence_estimator_editor::FluenceEstimatorEditor, i32_editor::I32Editor,
        isometry_option_editor::IsometryOptionEditor, length_editor::LengthEditor,
        length_option_editor::LengthOptionEditor, linear_density_editor::LinearDensityEditor,
        material_editor::MaterialEditor, splitter_type_editor::SplitterTypeEditor,
        string_editor::StringEditor, vec2_editor::Vec2Editor, vec3_editor::Vec3Editor,
    },
};
use dioxus::prelude::*;
use opossum_core::{
    prelude::{Properties, Property, Proptype},
    properties::proptype::AssetRef,
    types::api_types::{NodeEditorPanel, NodeInfo},
};
use uuid::Uuid;

#[component]
pub fn PropertiesEditor(
    node_id: Memo<Uuid>,
    node_properties_sig: ReadSignal<Properties>,
    node_info_sig: ReadSignal<NodeInfo>,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let editor_inputs = if node_info_sig.read().uuid == *node_id.read() {
        let mut editor_inputs = Vec::<Result<VNode, RenderError>>::new();
        for (property_key, property) in node_properties_sig.read().iter() {
            if let Some(editor) =
                get_editor(node_id, property, property_key.clone(), on_change, readonly)
            {
                editor_inputs.push(editor);
            }
        }
        editor_inputs
    } else {
        vec![]
    };
    rsx! {
        AccordionItem {
            elements: editor_inputs,
            header: "Properties",
            header_id: "propertyHeading",
            parent_id: "accordionNodeConfig",
            content_id: content_id_for_panel(NodeEditorPanel::Properties),
            level: 1,
        }
    }
}

fn get_editor(
    node_id: Memo<Uuid>,
    property: &Property,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Option<Element> {
    if let Some(editor) =
        get_primitive_editor(node_id, property, property_key.clone(), on_change, readonly)
    {
        return Some(editor);
    }

    if let Some(editor) =
        get_optical_editor(node_id, property, property_key.clone(), on_change, readonly)
    {
        return Some(editor);
    }
    get_geometric_editor(node_id, property, property_key, on_change, readonly)
}

fn get_primitive_editor(
    node_id: Memo<Uuid>,
    property: &Property,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Option<Element> {
    match property.prop().clone() {
        Proptype::String(s) => Some(rsx! {
            StringEditor {
                node_id,
                s,
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::I32(int32) => Some(rsx! {
            I32Editor {
                node_id,
                int32,
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::F64(float64) => Some(rsx! {
            F64Editor {
                node_id,
                float64,
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::Bool(b) => Some(rsx! {
            BoolEditor {
                node_id,
                b,
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::Vec2(vector) => Some(rsx! {
            Vec2Editor {
                node_id,
                vector,
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::Vec3(vector) => Some(rsx! {
            Vec3Editor {
                node_id,
                vector,
                property_key,
                on_change,
                readonly,
            }
        }),
        _ => None,
    }
}

/// Editors for properties describing an optical behaviour.
fn get_optical_editor(
    node_id: Memo<Uuid>,
    property: &Property,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Option<Element> {
    match property.prop().clone() {
        Proptype::SplittingConfigBuilder(splitting_config_builder) => Some(rsx! {
            SplitterTypeEditor {
                node_id,
                splitting_config_builder,
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::FilterTypeBuilder(filter_type_builder) => Some(rsx! {
            FilterTypeEditor {
                node_id,
                filter_type_builder,
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::FluenceEstimator(fluence_estimator) => Some(rsx! {
            FluenceEstimatorEditor {
                node_id,
                fluence_estimator,
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::LinearDensity(linear_density) => Some(rsx! {
            LinearDensityEditor {
                node_id,
                linear_density,
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::LightDataBuilder(_light_data_builder) => Some(rsx! { "no longer available" }),
        // A document is hydrated on load, so a material reaching the editor is normally embedded.
        // A bare id can only show up for a registry material this GUI cannot resolve yet - it is
        // named rather than skipped, so the property never silently disappears from the panel.
        Proptype::Material(AssetRef::Inline(material)) => Some(rsx! {
            MaterialEditor {
                node_id,
                material,
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::Material(AssetRef::Id(material_id)) => Some(rsx! {
            div { class: "accordion-content-wrapper-div",
                "Material from registry ({material_id})"
            }
        }),
        _ => None,
    }
}

fn get_geometric_editor(
    node_id: Memo<Uuid>,
    property: &Property,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Option<Element> {
    match property.prop().clone() {
        Proptype::Length(length) => Some(rsx! {
            LengthEditor {
                node_id,
                length,
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::Curvature(curvature) => Some(rsx! {
            CurvatureEditor {
                node_id,
                curvature,
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::LengthOption(length_opt) => Some(rsx! {
            LengthOptionEditor {
                node_id,
                length_opt,
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::Isometry(isometry) => Some(rsx! {
            IsometryOptionEditor {
                node_id,
                isometry: isometry.unwrap_or_default(),
                property_key,
                on_change,
                readonly,
            }
        }),
        Proptype::Angle(angle) => Some(rsx! {
            AngleEditor {
                node_id,
                angle,
                property_key,
                on_change,
                readonly,
            }
        }),
        _ => None,
    }
}

/// Creates an `EventHandler` for saving a property change, which updates the signal and calls the `on_change` handler with a `NodeChangeEvent`.
/// This function can be used in property editors to handle changes to properties and ensure that the UI updates accordingly.
/// # Arguments
/// * `sig` - A `Signal` that holds the current value of the property being edited. This signal will be updated when the property changes This signal should be defined in the calling component.
/// * `property_key` - The key of the property being edited. This is used in the `NodeChangeEvent` to specify which property has changed.
/// * `change_handler` - An `EventHandler` that will be called with a `NodeChangeEvent` when the property changes. This is typically the `on_change` handler passed down from the parent component.
/// * `node_id` - A `ReadSignal` that provides the ID of the node whose property is being edited. This is used in the `NodeChangeEvent` to specify which node has changed.
/// # Returns
/// An `EventHandler` that can be used in the property editor to handle changes to the property.
pub fn on_save_proptype_handler<T: Into<Proptype> + PartialEq + Clone + 'static>(
    mut sig: Signal<T>,
    property_key: String,
    change_handler: EventHandler<NodeChangeEvent>,
    node_id: ReadSignal<Uuid>,
) -> EventHandler<T> {
    EventHandler::new(move |new_prop: T| {
        if new_prop != *sig.read() {
            change_handler.call(NodeChangeEvent {
                node_id: *node_id.read(),
                action: NodeChangeAction::Property(property_key.clone(), new_prop.clone().into()),
            });
            sig.set(new_prop);
        }
    })
}
