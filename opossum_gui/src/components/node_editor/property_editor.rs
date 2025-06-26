use crate::components::node_editor::{
    accordion::{AccordionItem, LabeledInput},
    node_editor_component::NodeChange, source_editor::LightDataEditor,
};
use dioxus::prelude::*;
use opossum_backend::{millimeter, Properties, Property, Proptype, RefrIndexConst, RefractiveIndexType};
use uom::si::{f64::Length, length::millimeter};
use inflector::Inflector;

#[component]
pub fn PropertiesEditor(node_properties: Properties, node_change: Signal<Option<NodeChange>>) -> Element{
    let mut editor_inputs = Vec::<Result<VNode, RenderError>>::new();
    for (property_key, property) in node_properties.iter(){
        println!("prop desc: {}, prop desc: {}", property.description(), property_key);
        editor_inputs.push(rsx!{PropertyEditor{prop_type: property.prop().clone(), property_key: property_key.clone(), node_change }});
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
    let prop_type_opt = Signal::new(None::<Proptype>);

    use_effect({
        let property_key = property_key.clone();
        move || {
        if let Some(prop_type) = prop_type_opt(){
            node_change.set(Some(NodeChange::Property(
                property_key.clone(),
                serde_json::to_value(prop_type)
                .unwrap(),
            )))
        }
    }});

    match prop_type{
        Proptype::String(_) => {println!("not yet implemented"); rsx!{}},
        Proptype::I32(_) => {println!("not yet implemented"); rsx!{}},
        Proptype::F64(_) => {println!("not yet implemented"); rsx!{}},
        Proptype::Bool(_) => {println!("not yet implemented"); rsx!{}},
        Proptype::LightData(light_data) => {println!("not yet implemented"); rsx!{}},
        Proptype::FilterType(filter_type) => {println!("not yet implemented"); rsx!{}},
        Proptype::SplitterType(splitting_config) => {println!("not yet implemented"); rsx!{}},
        Proptype::SpectrometerType(spectrometer_type) => {println!("not yet implemented"); rsx!{}},
        Proptype::Metertype(metertype) => {println!("not yet implemented"); rsx!{}},
        Proptype::Uuid(uuid) => {println!("not yet implemented"); rsx!{}},
        Proptype::Aperture(aperture) => {println!("not yet implemented"); rsx!{}},
        Proptype::Spectrum(spectrum) => {println!("not yet implemented"); rsx!{}},
        Proptype::FluenceData(fluence_data) => {println!("not yet implemented"); rsx!{}},
        Proptype::FluenceEstimator(fluence_estimator) => {println!("not yet implemented"); rsx!{}},
        Proptype::WaveFrontData(wave_front_data) => {println!("not yet implemented"); rsx!{}},
        Proptype::RayPositionHistory(ray_position_histories) => {println!("not yet implemented"); rsx!{}},
        Proptype::GhostFocusHistory(ghost_focus_history) => {println!("not yet implemented"); rsx!{}},
        Proptype::NodeReport(node_report) => {println!("not yet implemented"); rsx!{}},
        Proptype::LinearDensity(quantity) => {println!("not yet implemented"); rsx!{}},
        Proptype::Fluence(quantity) => {println!("not yet implemented"); rsx!{}},
        Proptype::WfLambda(_, quantity) => {println!("not yet implemented"); rsx!{}},
        Proptype::Length(quantity) => rsx!{LengthEditor{length: quantity,
    prop_type,
    property_key, 
    prop_type_opt}},
        Proptype::LengthOption(quantity) => {println!("not yet implemented"); rsx!{}},
        Proptype::Energy(quantity) => {println!("not yet implemented"); rsx!{}},
        Proptype::Angle(quantity) => {println!("not yet implemented"); rsx!{}},
        Proptype::RefractiveIndex(refractive_index_type) => {println!("not yet implemented"); rsx!{}},
        Proptype::Isometry(isometry) => {println!("not yet implemented"); rsx!{}},
        Proptype::Vec3(matrix) => {println!("not yet implemented"); rsx!{}},
        Proptype::HitMap(hit_map) => {println!("not yet implemented"); rsx!{}},
        Proptype::Vec2(matrix) => {println!("not yet implemented"); rsx!{}},
        Proptype::LightDataBuilder(light_data_builder) => rsx!{
            LightDataEditor {
                        light_data_builder_opt: light_data_builder, prop_type_opt
                        }},
        _ => {println!("not yet implemented"); rsx!{}},
    }
}


#[component]
pub fn LengthEditor(
    length: Length,
    prop_type: Proptype,
    property_key: String, 
    prop_type_opt: Signal<Option<Proptype>>
) -> Element{

    rsx!{
        LabeledInput {
            id: format!("lengthProperty{property_key}").to_camel_case(),
            label: format!("{} in mm", property_key.to_sentence_case()),
            value: format!("{}", length.get::<millimeter>()),
            r#type: "number",
            onchange: Some(use_on_length_input_change(prop_type_opt)),
        }
    }
}

fn use_on_length_input_change(
    mut signal: Signal<Option<Proptype>>,
) -> Callback<Event<FormData>> {
    use_callback(move |e: Event<FormData>| {
        if let Ok(length) = e.data.value().parse::<f64>() {
            signal.set(Some(Proptype::Length(millimeter!(length))));
        }
    })
}