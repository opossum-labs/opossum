#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::node_editor::{
    accordion::{AccordionItem, LabeledInput, LabeledSelect},
    node_editor_component::NodeChange,
    source_editor::{CallbackWrapper, LightDataEditor},
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{millimeter, nanometer, Properties, Proptype};
use uom::si::{
    f64::Length,
    length::{millimeter, nanometer},
};

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
    let prop_type_sig = Signal::new(prop_type.clone());

    use_effect({
        let property_key = property_key.clone();
        move || {
            node_change.set(Some(NodeChange::Property(
                property_key.clone(),
                serde_json::to_value(prop_type_sig.read().clone()).unwrap(),
            )));
        }
    });

    match prop_type {
        Proptype::String(_) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::I32(_) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::F64(_) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::Bool(_) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::LightData(_light_data) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::FilterType(_filter_type) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::SplitterType(_splitting_config) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::SpectrometerType(_spectrometer_type) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::Metertype(_metertype) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::Uuid(_uuid) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::Aperture(_aperture) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::Spectrum(_spectrum) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::FluenceData(_fluence_data) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::FluenceEstimator(_fluence_estimator) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::WaveFrontData(_wave_front_data) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::RayPositionHistory(_ray_position_histories) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::GhostFocusHistory(_ghost_focus_history) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::NodeReport(_node_report) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::LinearDensity(_quantity) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::Fluence(_quantity) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::WfLambda(_, _quantity) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::Length(quantity) => rsx! {
            LengthEditor {
                length: quantity,
                prop_type,
                property_key,
                prop_type_sig,
            }
        },
        Proptype::LengthOption(quantity) => rsx! {
            AlignmentWavelengthEditor {
                length_option: quantity,
                prop_type,
                property_key,
                prop_type_sig,
            }
        },
        Proptype::Energy(_quantity) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::Angle(_quantity) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::RefractiveIndex(_refractive_index_type) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::Isometry(_isometry) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::Vec3(_matrix) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::HitMap(_hit_map) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::Vec2(_matrix) => {
            println!("not yet implemented");
            rsx! {}
        }
        Proptype::LightDataBuilder(light_data_builder) => rsx! {
            LightDataEditor { light_data_builder_opt: light_data_builder, prop_type_sig }
        },
        _ => {
            println!("not yet implemented");
            rsx! {}
        }
    }
}

#[component]
pub fn LengthEditor(
    length: Length,
    prop_type: Proptype,
    property_key: String,
    prop_type_sig: Signal<Proptype>,
) -> Element {
    rsx! {
        LabeledInput {
            id: format!("lengthProperty{property_key}").to_camel_case(),
            label: format!("{} in mm", property_key.to_sentence_case()),
            value: format!("{}", length.get::<millimeter>()),
            r#type: "number",
            onchange: on_length_input_change(prop_type_sig),
        }
    }
}

fn on_length_input_change(mut signal: Signal<Proptype>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(length) = e.data.value().parse::<f64>() {
            signal.set(Proptype::Length(millimeter!(length)));
        }
    })
}

#[component]
pub fn AlignmentWavelengthEditor(
    length_option: Option<Length>,
    prop_type: Proptype,
    property_key: String,
    prop_type_sig: Signal<Proptype>,
) -> Element {
    let mut alignment_select = Signal::new(nanometer!(1054.));
    let select_id = format!("lengthProperty{property_key}").to_camel_case();
    let select_label = property_key.to_sentence_case();

    if let Proptype::LengthOption(Some(length)) = &*prop_type_sig.read() {
        rsx! {
            LabeledSelect {
                id: select_id,
                label: select_label,
                options: vec![
                    (false, "As in light definition".to_owned()),
                    (true, "Choose specific".to_owned()),
                ],
                onchange: move |_: Event<FormData>| prop_type_sig.set(Proptype::LengthOption(None)),
            }
            LabeledInput {
                id: format!("lengthOptionProperty{property_key}").to_camel_case(),
                label: format!("{} in nm", property_key.to_sentence_case()),
                value: format!("{}", length.get::<nanometer>()),
                r#type: "number",
                onchange: CallbackWrapper::new(move |e: Event<FormData>| {
                    if let Ok(length) = e.data.value().parse::<f64>() {
                        prop_type_sig.set(Proptype::LengthOption(Some(nanometer!(length))));
                        alignment_select.set(nanometer!(length));
                    }
                }),
            }
        }
    } else {
        rsx! {
            LabeledSelect {
                id: select_id,
                label: select_label,
                options: vec![
                    (true, "As in light definition".to_owned()),
                    (false, "Choose specific".to_owned()),
                ],
                onchange: move |_: Event<FormData>| {
                    prop_type_sig.set(Proptype::LengthOption(Some(alignment_select())));
                },
            }
        }
    }
}
