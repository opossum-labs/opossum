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
    node_config_editor::NodeChangeAction,
    optical_node_editor::properties_editor::{
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
    let mut editor_inputs = Vec::<Result<VNode, RenderError>>::new();

    for (property_key, property) in &*node_properties_sig.read() {
        if let Some(editor) = get_editor(property.clone(), property_key.clone()) {
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

fn get_editor(property: Property, property_key: String) -> Option<Element> {
    match property.prop().clone() {
        Proptype::String(s) => Some(rsx! {
            StringEditor { s, property_key }
        }),
        Proptype::I32(int32) => Some(rsx! {
            I32Editor { int32, property_key }
        }),
        Proptype::F64(float64) => Some(rsx! {
            F64Editor { float64, property_key, property }
        }),
        Proptype::Bool(b) => Some(rsx! {
            BoolEditor { b, property_key }
        }),
        Proptype::SplittingConfigBuilder(splitting_config_builder) => Some(rsx! {
            SplitterTypeEditor {
                splitting_config_builder,
                property_key,
                property,
            }
        }),
        Proptype::FilterTypeBuilder(filter_type_builder) => Some(rsx! {
            FilterTypeEditor { filter_type_builder, property_key, property }
        }),
        Proptype::FluenceEstimator(fluence_estimator) => Some(rsx! {
            FluenceEstimatorEditor { fluence_estimator, property_key }
        }),
        Proptype::LinearDensity(linear_density) => Some(rsx! {
            LinearDensityEditor { linear_density, property_key }
        }),
        Proptype::Length(length) => Some(rsx! {
            LengthEditor { length, property_key }
        }),
        Proptype::Curvature(curvature) => Some(rsx! {
            CurvatureEditor { curvature, property_key }
        }),
        Proptype::LightDataBuilder(light_data_builder) => Some(rsx! {
            LightDataEditor {
                light_data_builder,
                property_key,
            }
        }),
        Proptype::LengthOption(length_opt) => Some(rsx! {
            LengthOptionEditor { length_opt, property_key }
        }),
        Proptype::Isometry(isometry) => Some(rsx! {
            IsometryOptionEditor { isometry: isometry.unwrap_or_default(), property_key }
        }),
        Proptype::Angle(angle) => Some(rsx! {
            AngleEditor { angle, property_key }
        }),
        Proptype::RefractiveIndex(ref_ind_type) => Some(rsx! {
            RefractiveIndexEditor { ref_ind_type, property_key }
        }),
        Proptype::Vec2(vector) => Some(rsx! {
            Vec2Editor { vector, property_key }
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
) {
    let node_change_handle = use_coroutine_handle::<NodeChangeAction>();
    use_update_signal_with_reactive_prop(prop_type_value.clone(), prop_type_value_sig);
    use_effect({
        let property_key = property_key.to_owned();
        move || {
            if prop_type_value != *prop_type_value_sig.read() {
                node_change_handle.send(NodeChangeAction::Property(
                    property_key.clone(),
                    prop_type_value_sig.read().clone().into(),
                ));
            }
        }
    });
}

pub fn use_update_signal_with_reactive_prop<T: PartialEq + Clone>(
    prop: T,
    mut prop_signal: Signal<T>,
) {
    use_effect({
        use_reactive!(|(prop,)| {
            if *prop_signal.peek() != prop {
                prop_signal.set(prop);
            }
        })
    });
}
