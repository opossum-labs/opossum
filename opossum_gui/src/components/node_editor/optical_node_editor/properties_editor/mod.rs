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
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
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
use opossum_core::prelude::{Properties, Property, Proptype};
use uuid::Uuid;

#[component]
pub fn PropertiesEditor(
    node_id: Uuid,
    node_properties_sig: Signal<Properties>,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut editor_inputs = Vec::<Result<VNode, RenderError>>::new();

    for (property_key, property) in &*node_properties_sig.read() {
        if let Some(editor) = get_editor(
            node_id,
            property.clone(),
            property_key.clone(),
            on_change.clone(),
        ) {
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

// Helper: creates the suitable editor
fn get_editor(
    node_id: Uuid,
    property: Property,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Option<Element> {
    match property.prop().clone() {
        Proptype::String(s) => Some(rsx! {
            StringEditor {
                node_id,
                s,
                property_key,
                on_change,
            }
        }),
        Proptype::I32(int32) => Some(rsx! {
            I32Editor {
                node_id,
                int32,
                property_key,
                on_change,
            }
        }),
        Proptype::F64(float64) => Some(rsx! {
            F64Editor {
                node_id,
                float64,
                property_key,
                property,
                on_change,
            }
        }),
        Proptype::Bool(b) => Some(rsx! {
            BoolEditor {
                node_id,
                b,
                property_key,
                on_change,
            }
        }),
        Proptype::SplittingConfigBuilder(splitting_config_builder) => Some(rsx! {
            SplitterTypeEditor {
                node_id,
                splitting_config_builder,
                property_key,
                property,
                on_change,
            }
        }),
        Proptype::FilterTypeBuilder(filter_type_builder) => Some(rsx! {
            FilterTypeEditor {
                node_id,
                filter_type_builder,
                property_key,
                property,
                on_change,
            }
        }),
        Proptype::FluenceEstimator(fluence_estimator) => Some(rsx! {
            FluenceEstimatorEditor {
                node_id,
                fluence_estimator,
                property_key,
                on_change,
            }
        }),
        Proptype::LinearDensity(linear_density) => Some(rsx! {
            LinearDensityEditor {
                node_id,
                linear_density,
                property_key,
                on_change,
            }
        }),
        Proptype::Length(length) => Some(rsx! {
            LengthEditor {
                node_id,
                length,
                property_key,
                on_change,
            }
        }),
        Proptype::Curvature(curvature) => Some(rsx! {
            CurvatureEditor {
                node_id,
                curvature,
                property_key,
                on_change,
            }
        }),
        Proptype::LightDataBuilder(light_data_builder) => Some(rsx! {
            LightDataEditor {
                node_id,
                light_data_builder,
                property_key,
                on_change,
            }
        }),
        Proptype::LengthOption(length_opt) => Some(rsx! {
            LengthOptionEditor {
                node_id,
                length_opt,
                property_key,
                on_change,
            }
        }),
        Proptype::Isometry(isometry) => Some(rsx! {
            IsometryOptionEditor {
                node_id,
                isometry: isometry.unwrap_or_default(),
                property_key,
                on_change,
            }
        }),
        Proptype::Angle(angle) => Some(rsx! {
            AngleEditor {
                node_id,
                angle,
                property_key,
                on_change,
            }
        }),
        Proptype::RefractiveIndex(ref_ind_type) => Some(rsx! {
            RefractiveIndexEditor {
                node_id,
                ref_ind_type,
                property_key,
                on_change,
            }
        }),
        Proptype::Vec2(vector) => Some(rsx! {
            Vec2Editor {
                node_id,
                vector,
                property_key,
                on_change,
            }
        }),
        // properties that should not be edited...
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

/// Dieser Hook wird von den spezifischen Editoren (z.B. F64Editor) aufgerufen.
/// Er wurde angepasst, um NodeChangeEvent via Callback zu senden, statt use_coroutine zu nutzen.
pub fn use_set_node_change_property<T: Into<Proptype> + PartialEq + Clone + 'static>(
    node_id: Uuid, // Hier sollte die "Lagging ID" übergeben werden!
    property_key: &str,
    prop_type_value: T,
    prop_type_value_sig: Signal<T>,
    on_change: EventHandler<NodeChangeEvent>, // NEU: Callback statt Coroutine
) {
    // Sync von außen nach innen (Standard Logik für Props->State)
    use_update_signal_with_reactive_prop(prop_type_value.clone(), prop_type_value_sig);

    use_effect({
        let property_key = property_key.to_owned();
        move || {
            // Wenn der User lokal etwas geändert hat (State != Prop)...
            if prop_type_value != *prop_type_value_sig.read() {
                // ... senden wir das Event.
                // WICHTIG: Die node_id, die hier reinkommt, muss vom Aufrufer
                // bereits via Lagging-ID-Pattern gesichert worden sein.
                on_change.call(NodeChangeEvent {
                    node_id,
                    action: NodeChangeAction::Property(
                        property_key.clone(),
                        prop_type_value_sig.read().clone().into(),
                    ),
                });
            }
        }
    });
}

pub fn use_update_signal_with_reactive_prop<T: PartialEq + Clone + 'static>(
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
