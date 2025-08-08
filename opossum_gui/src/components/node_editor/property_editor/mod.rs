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
mod light_data_editor;
mod linear_density_editor;
mod refractive_index_editor;
mod splitter_type_editor;
mod string_editor;
mod vec2_editor;

use crate::components::node_editor::{
    accordion::AccordionItem,
    node_editor_component::NodeChange,
    property_editor::{
        angle_editor::AngleEditor, bool_editor::BoolEditor, curvature_editor::CurvatureEditor,
        f64_editor::F64Editor, filter_type_editor::FilterTypeEditor,
        fluence_estimator_editor::FluenceEstimatorEditor, i32_editor::I32Editor,
        isometry_option_editor::IsometryOptionEditor, length_editor::LengthEditor,
        length_option_editor::LengthOptionEditor, light_data_editor::LightDataEditor,
        linear_density_editor::LinearDensityEditor, refractive_index_editor::RefractiveIndexEditor,
        splitter_type_editor::SplitterTypeEditor, string_editor::StringEditor,
        vec2_editor::Vec2Editor,
    },
};
use dioxus::prelude::*;
use opossum_backend::{Properties, Property, Proptype};

#[component]
pub fn PropertiesEditor(node_properties_sig: Signal<Properties>) -> Element {
    let node_change: Signal<Option<NodeChange>> = use_context::<Signal<Option<NodeChange>>>();
    let mut editor_inputs = Vec::<Result<VNode, RenderError>>::new();

    for (property_key, property) in &*node_properties_sig.read() {
        if let Some(editor) = get_editor(property.clone(), property_key.clone(), node_change) {
            editor_inputs.push(editor);
        }
    }
    if editor_inputs.is_empty() {
        rsx! {}
    } else {
        rsx! {
            AccordionItem {
                elements: editor_inputs,
                header: "Properties",
                header_id: "propertyHeading",
                parent_id: "accordionNodeConfig",
                content_id: "propertyCollapse",
            }
        }
    }
}

fn get_editor(
    property: Property,
    property_key: String,
    node_change: Signal<Option<NodeChange>>,
) -> Option<Element> {
    match property.prop().clone() {
        Proptype::String(s) => Some(rsx! {
            StringEditor { s, property_key, node_change }
        }),
        Proptype::I32(int32) => Some(rsx! {
            I32Editor { int32, property_key, node_change }
        }),
        Proptype::F64(float64) => Some(rsx! {
            F64Editor { float64, property_key, node_change }
        }),
        Proptype::Bool(b) => Some(rsx! {
            BoolEditor { b, property_key, node_change }
        }),
        Proptype::SplittingConfigBuilder(splitting_config_builder) => Some(rsx! {
            SplitterTypeEditor {
                splitting_config_builder,
                property_key,
                node_change,
                property,
            }
        }),
        Proptype::FilterTypeBuilder(filter_type_builder) => Some(rsx! {
            FilterTypeEditor {
                filter_type_builder,
                property_key,
                node_change,
                property,
            }
        }),
        Proptype::FluenceEstimator(fluence_estimator) => Some(rsx! {
            FluenceEstimatorEditor { fluence_estimator, property_key, node_change }
        }),
        Proptype::LinearDensity(linear_density) => Some(rsx! {
            LinearDensityEditor { linear_density, property_key, node_change }
        }),
        Proptype::Length(length) => Some(rsx! {
            LengthEditor { length, property_key, node_change }
        }),
        Proptype::Curvature(curvature) => Some(rsx! {
            CurvatureEditor { curvature, property_key, node_change }
        }),
        Proptype::LightDataBuilder(light_data_builder_opt) => Some(rsx! {
            LightDataEditor {
                light_data_builder: light_data_builder_opt.unwrap_or_default(),
                property_key,
                node_change,
            }
        }),
        Proptype::LengthOption(length_opt) => Some(rsx! {
            LengthOptionEditor { length_opt, property_key, node_change }
        }),
        Proptype::Isometry(isometry) => Some(rsx! {
            IsometryOptionEditor {
                isometry: isometry.unwrap_or_default(),
                property_key,
                node_change,
            }
        }),
        Proptype::Angle(angle) => Some(rsx! {
            AngleEditor { angle, property_key, node_change }
        }),
        Proptype::RefractiveIndex(ref_ind_type) => Some(rsx! {
            RefractiveIndexEditor { ref_ind_type, property_key, node_change }
        }),
        Proptype::Vec2(vector) => Some(rsx! {
            Vec2Editor { vector, property_key, node_change }
        }),
        //not used to change a node property
        Proptype::LightData(_)
        | Proptype::Uuid(_)
        | Proptype::FluenceData(_)
        | Proptype::SpectrometerType(_)
        | Proptype::WaveFrontData(_)
        | Proptype::RayPositionHistory(_)
        | Proptype::GhostFocusHistory(_)
        | Proptype::NodeReport(_)
        | Proptype::Fluence(_)
        | Proptype::WfLambda(_, _)
        | Proptype::Energy(_)
        | Proptype::Vec3(_)
        | Proptype::HitMap(_)
        | Proptype::Spectrum(_)
        | Proptype::Metertype(_)
        | Proptype::Aperture(_) => None,
    }
}

pub fn use_set_node_change_property<T: Into<Proptype> + PartialEq + Clone>(
    property_key: &str,
    prop_type_value: T,
    prop_type_value_sig: Signal<T>,
    mut node_change_sig: Signal<Option<NodeChange>>,
) {
    use_effect({
        let property_key = property_key.to_owned();
        move || {
            if prop_type_value != *prop_type_value_sig.read() {
                node_change_sig.set(Some(NodeChange::Property(
                    property_key.clone(),
                    prop_type_value_sig.read().clone().into(),
                )));
            }
        }
    });
}
