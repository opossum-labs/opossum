#![allow(clippy::derive_partial_eq_without_eq)]

mod angle_editor;
mod bool_editor;
mod curvature_editor;
mod f64_editor;
mod fluence_estimator_editor;
mod i32_editor;
mod isometry_option_editor;
mod length_editor;
mod length_option_editor;
mod light_data_editor;
mod linear_density_editor;
mod refractive_index_editor;
mod string_editor;
mod vec2_editor;
// mod splitter_type_editor;

use crate::components::node_editor::{
    accordion::AccordionItem,
    node_editor_component::NodeChange,
    property_editor::{
        angle_editor::AngleEditor,
        bool_editor::BoolEditor,
        curvature_editor::CurvatureEditor,
        f64_editor::F64Editor,
        fluence_estimator_editor::FluenceEstimatorEditor,
        i32_editor::I32Editor,
        isometry_option_editor::IsometryOptionEditor,
        length_editor::LengthEditor,
        length_option_editor::LengthOptionEditor,
        light_data_editor::LightDataEditor,
        // splitter_type_editor::SplitterTypeEditor,
        linear_density_editor::LinearDensityEditor,
        refractive_index_editor::RefractiveIndexEditor,
        string_editor::StringEditor,
        vec2_editor::Vec2Editor,
    },
};
use dioxus::prelude::*;
use opossum_backend::{Properties, Proptype};

#[component]
pub fn PropertiesEditor(
    node_properties: Properties,
    node_change: Signal<Option<NodeChange>>,
) -> Element {
    let mut editor_inputs = Vec::<Result<VNode, RenderError>>::new();

    for (property_key, property) in &node_properties {
        if let Some(editor) = get_editor(property.prop().clone(), property_key.clone(), node_change)
        {
            editor_inputs.push(editor);
        }
        // editor_inputs.push(get_editor(property.prop().clone(),property_key.clone(), node_change)

        //     rsx! {
        //     PropertyEditor {
        //         prop_type: property.prop().clone(),
        //         property_key: property_key.clone(),
        //         node_change,
        //     }
        // }
        // );
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

// #[component]
// pub fn PropertyEditor(
//     prop_type: Proptype,
//     property_key: String,
//     node_change: Signal<Option<NodeChange>>,
// ) -> Element {
//     let prop_type_sig = use_signal(|| prop_type.clone());
//     // use_effect({
//     //     let property_key = property_key.clone();
//     //     move || {
//     //         node_change.set(Some(NodeChange::Property(
//     //             property_key.clone(),
//     //             prop_type_sig.read().clone(),
//     //         )));
//     //     }
//     // });

//     match prop_type {
//         Proptype::String(s) => rsx! {
//             StringEditor { s, property_key, prop_type_sig }
//         },
//         Proptype::I32(int32) => rsx! {
//             I32Editor { int32, property_key, prop_type_sig }
//         },
//         Proptype::F64(float64) => rsx! {
//             F64Editor { float64, property_key, prop_type_sig }
//         },
//         Proptype::Bool(b) => rsx! {
//             BoolEditor { b, property_key, prop_type_sig }
//         },
//         Proptype::SplitterType(_splitting_config) => {
//             println!("splittertype not yet implemented");
//             rsx! {}
//         }
//         Proptype::FilterType(_spectrometer_type) => {
//             println!("filtertype not yet implemented");
//             rsx! {}
//         }
//         Proptype::FluenceEstimator(fluence_estimator) => rsx! {
//             FluenceEstimatorEditor { fluence_estimator, property_key, prop_type_sig }
//         },
//         Proptype::LinearDensity(linear_density) => rsx! {
//             LinearDensityEditor { linear_density, property_key, prop_type_sig }
//         },
//         Proptype::Length(length) => rsx! {
//             LengthEditor { length, property_key, node_change }
//         },
//         Proptype::Curvature(curvature) => rsx! {
//             CurvatureEditor { curvature, property_key, prop_type_sig }
//         },
//         Proptype::LightDataBuilder(light_data_builder_opt) => rsx! {
//             LightDataEditor {
//                 light_data_builder: light_data_builder_opt.unwrap_or_default(),
//                 property_key,
//                 node_change,
//             }
//         },
//         Proptype::LengthOption(length_opt) => rsx! {
//             LengthOptionEditor { length_opt, property_key, prop_type_sig }
//         },
//         Proptype::Isometry(_) => rsx! {
//             IsometryOptionEditor { property_key, prop_type_sig }
//         },
//         Proptype::Angle(angle) => rsx! {
//             AngleEditor { angle, property_key, prop_type_sig }
//         },
//         Proptype::RefractiveIndex(refractive_index_type) => rsx! {
//             RefractiveIndexEditor {
//                 property_key,
//                 prop_type_sig,
//                 ref_ind_sig: Signal::new(refractive_index_type),
//             }
//         },
//         Proptype::Vec2(vector) => rsx! {
//             Vec2Editor { vector, property_key, prop_type_sig }
//         },
//         //not used to change a node property
//         Proptype::LightData(_)
//         | Proptype::Uuid(_)
//         | Proptype::FluenceData(_)
//         | Proptype::SpectrometerType(_)
//         | Proptype::WaveFrontData(_)
//         | Proptype::RayPositionHistory(_)
//         | Proptype::GhostFocusHistory(_)
//         | Proptype::NodeReport(_)
//         | Proptype::Fluence(_)
//         | Proptype::WfLambda(_, _)
//         | Proptype::Energy(_)
//         | Proptype::Vec3(_)
//         | Proptype::HitMap(_)
//         | Proptype::Spectrum(_)
//         | Proptype::Metertype(_)
//         | Proptype::Aperture(_) => no_additional_props_display(),
//     }
// }

fn get_editor(
    prop_type: Proptype,
    property_key: String,
    node_change: Signal<Option<NodeChange>>,
) -> Option<Element> {
    match prop_type {
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
        Proptype::SplitterType(_splitting_config) => {
            println!("splittertype not yet implemented");
            Some(rsx! {})
        }
        Proptype::FilterType(_spectrometer_type) => {
            println!("filtertype not yet implemented");
            Some(rsx! {})
        }
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
