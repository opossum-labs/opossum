#![allow(clippy::derive_partial_eq_without_eq)]

mod bool_editor;
mod f64_editor;
mod i32_editor;
mod isometry_option_editor;
mod length_editor;
mod length_option_editor;
mod light_data_editor;
mod linear_density_editor;
mod refractive_index_editor;
// mod splitter_type_editor;

use crate::components::node_editor::{
    accordion::AccordionItem,
    node_editor_component::NodeChange,
    property_editor::{
        bool_editor::BoolEditor,
        f64_editor::F64Editor,
        i32_editor::I32Editor,
        isometry_option_editor::IsometryOptionEditor,
        length_editor::LengthEditor,
        length_option_editor::AlignmentWavelengthEditor,
        light_data_editor::LightDataEditor,
        // splitter_type_editor::SplitterTypeEditor,
        linear_density_editor::LinearDensityEditor,
        refractive_index_editor::RefractiveIndexEditor,
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
        editor_inputs.push(rsx! {
            PropertyEditor {
                prop_type: property.prop().clone(),
                property_key: property_key.clone(),
                node_change,
            }
        });
    }
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

#[component]
pub fn PropertyEditor(
    prop_type: Proptype,
    property_key: String,
    node_change: Signal<Option<NodeChange>>,
) -> Element {
    let prop_type_sig = use_signal(|| prop_type.clone());

    use_effect({
        let property_key = property_key.clone();
        move || {
            node_change.set(Some(NodeChange::Property(
                property_key.clone(),
                serde_json::to_value(prop_type_sig.read().clone()).unwrap(),
            )));
        }
    });

    match &prop_type {
        Proptype::String(_) => {
            println!("String not yet implemented");
            rsx! {}
        }
        Proptype::I32(int32) => rsx! {
            I32Editor { int32: *int32, property_key, prop_type_sig }
        },
        Proptype::F64(float64) => rsx! {
            F64Editor { float64: *float64, property_key, prop_type_sig }
        },
        Proptype::Bool(b) => rsx! {
            BoolEditor { b: *b, property_key, prop_type_sig }
        },
        Proptype::LightData(_light_data) => {
            println!("Lightdata not yet implemented");
            rsx! {}
        }
        Proptype::FilterType(_filter_type) => {
            println!("FilterType not yet implemented");
            rsx! {}
        }
        Proptype::SplitterType(_splitting_config) => rsx! {},
        Proptype::SpectrometerType(_spectrometer_type) => {
            println!("spectrometertype not yet implemented");
            rsx! {}
        }
        // Proptype::Metertype(_metertype) => {
        //     println!("Metertype not yet implemented");
        //     rsx! {}
        // }
        Proptype::Uuid(_uuid) => {
            println!("Uuid not yet implemented");
            rsx! {}
        }
        Proptype::Aperture(_aperture) => {
            println!("Aperture not yet implemented");
            rsx! {}
        }
        Proptype::Spectrum(_spectrum) => {
            println!("Spectrum not yet implemented");
            rsx! {}
        }
        Proptype::FluenceData(_fluence_data) => {
            println!("FluenceData not yet implemented");
            rsx! {}
        }
        Proptype::FluenceEstimator(_fluence_estimator) => {
            println!("FLuenceEstimator not yet implemented");
            rsx! {}
        }
        Proptype::WaveFrontData(_wave_front_data) => {
            println!("WaveFrontData not yet implemented");
            rsx! {}
        }
        Proptype::RayPositionHistory(_ray_position_histories) => {
            println!("RayPositionHistory not yet implemented");
            rsx! {}
        }
        Proptype::GhostFocusHistory(_ghost_focus_history) => {
            println!("GhostFocusHistory not yet implemented");
            rsx! {}
        }
        Proptype::NodeReport(_node_report) => {
            println!("NodeReport not yet implemented");
            rsx! {}
        }
        Proptype::LinearDensity(quantity) => rsx! {
            LinearDensityEditor {
                linear_density: *quantity,
                property_key,
                prop_type_sig,
            }
        },
        Proptype::Fluence(_quantity) => {
            println!("Fluence not yet implemented");
            rsx! {}
        }
        Proptype::WfLambda(_, _quantity) => {
            println!("WfLambda not yet implemented");
            rsx! {}
        }
        Proptype::Length(quantity) => rsx! {
            LengthEditor { length: *quantity, property_key, prop_type_sig }
        },
        Proptype::LightDataBuilder(light_data_builder_opt) => rsx! {
            LightDataEditor {
                light_data_builder: light_data_builder_opt.clone().unwrap_or_default(),
                prop_type_sig,
            }
        },
        Proptype::LengthOption(_) => rsx! {
            AlignmentWavelengthEditor { property_key, prop_type_sig }
        },
        Proptype::Isometry(_) => rsx! {
            IsometryOptionEditor { property_key, prop_type_sig }
        },
        Proptype::Energy(_quantity) => {
            println!("Energy not yet implemented");
            rsx! {}
        }
        Proptype::Angle(_quantity) => {
            println!("Angle not yet implemented");
            rsx! {}
        }
        Proptype::RefractiveIndex(refractive_index_type) => rsx! {
            RefractiveIndexEditor {
                property_key,
                prop_type_sig,
                ref_ind_sig: Signal::new(refractive_index_type.clone()),
            }
        },
        Proptype::Vec3(_matrix) => {
            println!("Vec3 not yet implemented");
            rsx! {}
        }
        Proptype::HitMap(_hit_map) => {
            println!("HitMap not yet implemented");
            rsx! {}
        }
        Proptype::Vec2(_matrix) => {
            println!("Vec2 not yet implemented");
            rsx! {}
        }
        _ => rsx! {
            div { "No additional properties necessary" }
        },
    }
}
