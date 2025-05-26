use std::collections::HashMap;

use crate::{api, components::scenery_editor::node::NodeElement, HTTP_API_CLIENT, OPOSSUM_UI_LOGS};
use dioxus::{html::geometry::euclid::num::Zero, prelude::*};
use nalgebra::Point3;
use opossum_backend::energy_data_builder::EnergyDataBuilder;
use opossum_backend::light_data_builder::{self, LightDataBuilder};
use opossum_backend::ray_data_builder::RayDataBuilder;
use opossum_backend::{
    joule, millimeter, nanometer, Fluence, Hexapolar, Isometry, LaserLines, NodeAttr, Proptype, UniformDist
};
use serde_json::Value;
use uom::si::f64::{Angle, RadiantExposure};
use uom::si::radiant_exposure::joule_per_square_centimeter;
use uom::si::{angle::degree, f64::Length, length::meter};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum NodeChange {
    Name(String),
    LIDT(Fluence),
    TranslationX(Length),
    TranslationY(Length),
    TranslationZ(Length),
    RotationRoll(Angle),
    RotationPitch(Angle),
    RotationYaw(Angle),
    Inverted(bool),
    NodeConst(String), // AlignLikeNodeAtDistance(Uuid, Length),
    Property(String, Value),
    // SourceWavelength(Length),
}

#[component]
pub fn NodeEditor(mut node: Signal<Option<NodeElement>>) -> Element {
    let mut node_change = use_context_provider(|| Signal::new(None::<NodeChange>));

    let geom_light_data = LightDataBuilder::Geometric(RayDataBuilder::Collimated {
        pos_dist: Hexapolar::new(millimeter!(5.), 5).unwrap().into(),
        energy_dist: UniformDist::new(joule!(1.)).unwrap().into(),
        spect_dist: LaserLines::new(vec![(nanometer!(1000.0), 1.0)])
            .unwrap()
            .into(),
    });
    let energy_light_data = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
        vec![(nanometer!(633.0), joule!(1.0))],
        nanometer!(1.0),
    ));
    let mut light_data_builder = HashMap::<String, LightDataBuilder>::new();
    light_data_builder.insert("Rays".to_string(), geom_light_data);
    light_data_builder.insert("Energy".to_string(), energy_light_data);
    let mut source_type = use_signal(|| None::<LightDataBuilder>);

    let active_node_opt = node();
    use_effect(move || {
        let node_change_opt = node_change.read().clone();
        let mut node = node.clone();
        if let (Some(node_changed), Some(mut active_node)) =
            (node_change_opt, active_node_opt.clone())
        {
            match node_changed {
                NodeChange::Name(name) => {
                    spawn(async move {
                        if let Err(err_str) = api::update_node_name(
                            &HTTP_API_CLIENT(),
                            active_node.id(),
                            name.clone(),
                        )
                        .await
                        {
                            OPOSSUM_UI_LOGS.write().add_log(&err_str);
                        } else {
                            active_node.set_name(name);
                            node.set(Some(active_node));
                        }
                    });
                }
                NodeChange::LIDT(lidt) => {
                    spawn(async move {
                        if let Err(err_str) = api::update_node_lidt(
                            &HTTP_API_CLIENT(),
                            active_node.id(),
                            lidt.clone(),
                        )
                        .await
                        {
                            OPOSSUM_UI_LOGS.write().add_log(&err_str);
                        };
                    });
                }
                NodeChange::TranslationX(x_trans) => {
                    spawn(async move {
                        if let Err(err_str) = api::update_node_translation(
                            &HTTP_API_CLIENT(),
                            active_node.id(),
                            (x_trans, 0),
                        )
                        .await
                        {
                            OPOSSUM_UI_LOGS.write().add_log(&err_str);
                        };
                    });
                }
                NodeChange::TranslationY(y_trans) => {
                    spawn(async move {
                        if let Err(err_str) = api::update_node_translation(
                            &HTTP_API_CLIENT(),
                            active_node.id(),
                            (y_trans, 1),
                        )
                        .await
                        {
                            OPOSSUM_UI_LOGS.write().add_log(&err_str);
                        };
                    });
                }
                NodeChange::TranslationZ(z_trans) => {
                    spawn(async move {
                        if let Err(err_str) = api::update_node_translation(
                            &HTTP_API_CLIENT(),
                            active_node.id(),
                            (z_trans, 2),
                        )
                        .await
                        {
                            OPOSSUM_UI_LOGS.write().add_log(&err_str);
                        };
                    });
                }
                NodeChange::RotationRoll(roll) => {
                    spawn(async move {
                        if let Err(err_str) = api::update_node_rotation(
                            &HTTP_API_CLIENT(),
                            active_node.id(),
                            (roll, 0),
                        )
                        .await
                        {
                            OPOSSUM_UI_LOGS.write().add_log(&err_str);
                        };
                    });
                }
                NodeChange::RotationPitch(pitch) => {
                    spawn(async move {
                        if let Err(err_str) = api::update_node_rotation(
                            &HTTP_API_CLIENT(),
                            active_node.id(),
                            (pitch, 1),
                        )
                        .await
                        {
                            OPOSSUM_UI_LOGS.write().add_log(&err_str);
                        };
                    });
                }
                NodeChange::RotationYaw(yaw) => {
                    spawn(async move {
                        if let Err(err_str) = api::update_node_rotation(
                            &HTTP_API_CLIENT(),
                            active_node.id(),
                            (yaw, 2),
                        )
                        .await
                        {
                            OPOSSUM_UI_LOGS.write().add_log(&err_str);
                        };
                    });
                }
                NodeChange::Property(key, prop) => {
                    spawn(async move {
                        if let Err(err_str) = api::update_node_property(
                            &HTTP_API_CLIENT(),
                            active_node.id(),
                            (key, prop),
                        )
                        .await
                        {
                            OPOSSUM_UI_LOGS.write().add_log(&err_str);
                        };
                    });
                }
                // NodeChange::SourceWavelength(new_wvl) => {
                //     let mut source_light_data_builder = source_type.read().clone();
                //     if let Some(LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(ref mut wvl, res))) = source_light_data_builder{
                //         *wvl = vec![(new_wvl, wvl[0].1)];
                //         spawn(async move {
                //             if let Err(err_str) = api::update_node_property(
                //                 &HTTP_API_CLIENT(),
                //                 active_node.id(),
                //                 ("light data".to_owned(), serde_json::to_value(source_light_data_builder).unwrap())
                //             )
                //             .await
                //             {
                //                 OPOSSUM_UI_LOGS.write().add_log(&err_str);
                //             };
                //         });
                //     }
                // }
                _ => {}
            }
        }
    });

    let resource_future = use_resource(move || async move {
        let node = node.read();
        if let Some(node) = &*(node) {
            match api::get_node_properties(&HTTP_API_CLIENT(), node.id()).await {
                Ok(node_attr) => Some(node_attr),
                Err(err_str) => {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    None
                }
            }
        } else {
            None
        }
    });

    if let Some(Some(node_attr)) = &*resource_future.read_unchecked() {
        rsx! {
            div {
                h6 { "Node Configuration" }
                div {
                    class: "accordion accordion-borderless bg-dark ",
                    id: "accordionGeneral",
                    div { class: "accordion-item bg-dark text-light",
                        h2 { class: "accordion-header", id: "generalHeading",
                            button {
                                class: "accordion-button collapsed bg-dark text-light",
                                r#type: "button",
                                "data-mdb-collapse-init": "",
                                "data-mdb-target": "#generalCollapse",
                                "aria-expanded": "false",
                                "aria-controls": "generalCollapse",
                                "General"
                            }
                        }
                        div {
                            id: "generalCollapse",
                            class: "accordion-collapse collapse  bg-dark",
                            "aria-labelledby": "generalHeading",
                            "data-mdb-parent": "#accordionGeneral",
                            div { class: "accordion-body  bg-dark",
                                NodePropInput {
                                    name: "NodeId".to_string(),
                                    placeholder: "Node ID".to_string(),
                                    node_change: NodeChange::NodeConst(format!("{}", node_attr.uuid())),
                                }
                                NodePropInput {
                                    name: "NodeType".to_string(),
                                    placeholder: "Node Type".to_string(),
                                    node_change: NodeChange::NodeConst(format!("{}", node_attr.node_type().to_string())),
                                }
                                NodePropInput {
                                    name: "NodeName".to_string(),
                                    placeholder: "Node Name".to_string(),
                                    node_change: NodeChange::Name(node_attr.name().to_string()),
                                }
                                NodePropInput {
                                    name: "LIDT".to_string(),
                                    placeholder: "LIDT in J/cm²".to_string(),
                                    node_change: NodeChange::LIDT(node_attr.lidt().clone()),
                                }
                            }
                        }
                    }
                }

                div {
                    hidden: {node_attr.node_type() != "source"},
                    class: "accordion accordion-borderless bg-dark ",
                    id: "accordionSource",
                    div { class: "accordion-item bg-dark text-light",
                        h2 { class: "accordion-header", id: "sourceHeading",
                            button {
                                class: "accordion-button collapsed bg-dark text-light",
                                r#type: "button",
                                "data-mdb-collapse-init": "",
                                "data-mdb-target": "#sourceCollapse",
                                "aria-expanded": "false",
                                "aria-controls": "sourceCollapse",
                                "Light Source"
                            }
                        }
                        div {
                            id: "sourceCollapse",
                            class: "accordion-collapse collapse  bg-dark",
                            "aria-labelledby": "sourceHeading",
                            "data-mdb-parent": "#accordionSource",
                            div { class: "accordion-body  bg-dark",
                            {
                                let node_props = node_attr.properties().clone();
                                if let Ok(light_data) = node_props.get("light data"){

                                    rsx!{
                                            div {
                                                class: "form-floating",
                                                select {
                                                    class: "form-select",
                                                    id: "selectSourceType",
                                                    "aria-label": "Select source type",
                                                    onchange: {
                                                        let light_data_builder = light_data_builder.clone();
                                                        move |e: Event<FormData>| {
                                                        source_type.set(light_data_builder.get(e.value().as_str()).cloned());
                                                        node_change.set(
                                                            Some(NodeChange::Property(
                                                                "light data".to_owned(), serde_json::to_value(Proptype::LightDataBuilder(light_data_builder.get(e.value().as_str()).cloned())).unwrap())));
                                                    }
                                                },

                                                {
                                                match light_data {
                                                        Proptype::LightDataBuilder(Some(light_data_builder)) => {
                                                            source_type.set(Some(light_data_builder.clone()));
                                                            match light_data_builder {
                                                                LightDataBuilder::Energy(_) => {
                                                                    rsx!{
                                                                        option { disabled: true, value: "None", "None" }
                                                                        option { selected: true, value: "Energy", "Energy" }
                                                                        option { value: "Rays", "Rays" }
                                                                    }
                                                                },
                                                                LightDataBuilder::Geometric(_) => {
                                                                    rsx!{
                                                                        option { disabled: true, value: "None", "None" }
                                                                        option { value: "Energy", "Energy" }
                                                                        option { selected: true, value: "Rays", "Rays" }
                                                                    }
                                                                },
                                                                _ => rsx!{
                                                                    option { selected:true, disabled: true, value: "None", "None" }
                                                                    option { value: "Energy", "Energy" }
                                                                    option { value: "Rays", "Rays" }
                                                                },
                                                            }
                                                        },
                                                        _ => rsx!{
                                                            option { selected:true, disabled: true, value: "None", "None" }
                                                            option { value: "Energy", "Energy" }
                                                            option { value: "Rays", "Rays" }
                                                        },
                                                    }
                                                    
                                                }
                                            }                                    
                                            label { r#for: "selectSourceType", "Source Type" },
                                        }
                                            {
                                                
                                                if let Proptype::LightDataBuilder(Some(LightDataBuilder::Geometric(ray_data_builder))) = light_data{
                                                    rsx!{
                                                        div {
                                                            class: "form-floating",
                                                            id: "selectRayType",
                                                            select {
                                                                class: "form-select",
                                                                "aria-label": "Select rays type",
                                                                onchange: {
                                                                    let mut light_data_builder = light_data_builder.clone();

                                                                    move |e| {
                                                                        match e.value().as_str() {
                                                                            "Collimated" => {
                                                                                light_data_builder.insert("Rays".to_string(), LightDataBuilder::Geometric(RayDataBuilder::Collimated {
                                                                                    pos_dist: Hexapolar::new(millimeter!(5.), 5).unwrap().into(),
                                                                                    energy_dist: UniformDist::new(joule!(1.)).unwrap().into(),
                                                                                    spect_dist: LaserLines::new(vec![(nanometer!(1000.0), 1.0)]).unwrap().into(),
                                                                                }));
                                                                            },
                                                                            "Point Source" => {
                                                                                light_data_builder.insert("Rays".to_string(), LightDataBuilder::Geometric(RayDataBuilder::PointSrc {
                                                                                    pos_dist: Hexapolar::new(millimeter!(5.), 5).unwrap().into(),
                                                                                    energy_dist: UniformDist::new(joule!(1.)).unwrap().into(),
                                                                                    spect_dist: LaserLines::new(vec![(nanometer!(1000.0), 1.0)]).unwrap().into(),
                                                                                }));
                                                                            },
                                                                            _ => {}
                                                                        }

                                                                    node_change.set(
                                                                        Some(NodeChange::Property(
                                                                            "light data".to_owned(), serde_json::to_value(Proptype::LightDataBuilder(light_data_builder.get("Rays").cloned())).unwrap())));
                                                                }},
                                                                {
                                                                    match ray_data_builder{
                                                                        RayDataBuilder::Collimated{..} => rsx!{
                                                                            option { selected: true, value: "Collimated", "Collimated" }
                                                                            option { value: "Point Source", "Point Source" }
                                                                        },
                                                                        RayDataBuilder::PointSrc{..} => rsx!{
                                                                            option { value: "Collimated", "Collimated" }
                                                                            option { selected: true, value: "Point Source", "Point Source" }
                                                                        },
                                                                        _ => rsx!{}
                                                                    }
                                                                }
                                                            }
                                                            label { r#for: "selectRayType", "Source Type" },
                                                        },
                                                    }
                                                }
                                                else{
                                                    rsx!{}
                                                }
                                            }
                                            // }
                                            // if let Some(source_type) = source_type() {
                                            //     match source_type {
                                            //         LightDataBuilder::Energy(energy_data) => {
                                            //             rsx! {
                                            //                 NodePropInput {
                                            //                     name: "Wavelength".to_string(),
                                            //                     placeholder: "Wavelength in nm".to_string(),
                                            //                     node_change: NodeChange::W(
                                            //                         "spectral lines".to_owned(),
                                            //                         serde_json::to_value(Proptype::SpectralLines(energy_data.spectral_lines().clone())).unwrap()
                                            //                     ),
                                            //                 }
                                            //                 NodePropInput {
                                            //                     name: "Spectral Width".to_string(),
                                            //                     placeholder: "Spectral Width in nm".to_string(),
                                            //                     node_change: NodeChange::Property(
                                            //                         "spectral width".to_owned(),
                                            //                         serde_json::to_value(Proptype::Length(energy_data.spectral_width().get::<nanometer>())).unwrap()
                                            //                     ),
                                            //                 }
                                            //             }
                                            //         },
                                            //         LightDataBuilder::Geometric(ray_data) => {
                                            //             rsx! {
                                            //                 NodePropInput {
                                            //                     name: "Position Distribution".to_string(),
                                            //                     placeholder: "Position Distribution".to_string(),
                                            //                     node_change: NodeChange::Property(
                                            //                         "position distribution".to_owned(),
                                            //                         serde_json::to_value(Proptype::PositionDistribution(ray_data.position_distribution().clone())).unwrap()
                                            //                     ),
                                            //                 }
                                            //                 NodePropInput {
                                            //                     name: "Energy Distribution".to_string(),
                                            //                     placeholder: "Energy Distribution".to_string(),
                                            //                     node_change: NodeChange::Property(
                                            //                         "energy distribution".to_owned(),
                                            //                         serde_json::to_value(Proptype::EnergyDistribution(ray_data.energy_distribution().clone())).unwrap()
                                            //                     ),
                                            //                 }
                                            //             }
                                            //         },
                                            //         _ => {}
                                            //     }
                                        // }
                                    }
                                }
                                else{
                                    rsx!{
                                        option { selected:true, disabled: true, value: "None", "None" }
                                        option { value: "Energy", "Energy" }
                                        option { value: "Rays", "Rays" }
                                    }
                                }
                                }
                            }
                        }
                    }
                }
                div {
                    class: "accordion accordion-borderless bg-dark ",
                    id: "accordionAlignment",
                    div { class: "accordion-item bg-dark text-light",
                        h2 { class: "accordion-header", id: "alignmentHeading",
                            button {
                                class: "accordion-button collapsed bg-dark text-light",
                                r#type: "button",
                                "data-mdb-collapse-init": "",
                                "data-mdb-target": "#alignmentCollapse",
                                "aria-expanded": "false",
                                "aria-controls": "alignmentCollapse",
                                "Alignment"
                            }
                        }
                        div {
                            id: "alignmentCollapse",
                            class: "accordion-collapse collapse  bg-dark",
                            "aria-labelledby": "alignmentHeading",
                            "data-mdb-parent": "#accordionAlignment",
                            div { class: "accordion-body  bg-dark",
                                NodePropInput {
                                    name: "XTranslation".to_string(),
                                    placeholder: "X Translation in m".to_string(),
                                    node_change: NodeChange::TranslationX(
                                        node_attr.alignment().as_ref().map_or(Length::zero(), |a| a.translation().x),
                                    ),
                                }
                                NodePropInput {
                                    name: "YTranslation".to_string(),
                                    placeholder: "Y Translation in m".to_string(),
                                    node_change: NodeChange::TranslationY(
                                        node_attr.alignment().as_ref().map_or(Length::zero(), |a| a.translation().y),
                                    ),
                                }
                                NodePropInput {
                                    name: "ZTranslation".to_string(),
                                    placeholder: "Z Translation in m".to_string(),
                                    node_change: NodeChange::TranslationZ(
                                        node_attr.alignment().as_ref().map_or(Length::zero(), |a| a.translation().z),
                                    ),
                                }
                                NodePropInput {
                                    name: "Roll".to_string(),
                                    placeholder: "Roll angle in degree".to_string(),
                                    node_change: NodeChange::RotationRoll(
                                        node_attr.alignment().as_ref().map_or(Angle::zero(), |a| a.rotation().x),
                                    ),
                                }
                                NodePropInput {
                                    name: "Pitch".to_string(),
                                    placeholder: "Pitch angle in degree".to_string(),
                                    node_change: NodeChange::RotationPitch(
                                        node_attr.alignment().as_ref().map_or(Angle::zero(), |a| a.rotation().y),
                                    ),
                                }
                                NodePropInput {
                                    name: "Yaw".to_string(),
                                    placeholder: "Yaw angle in degree".to_string(),
                                    node_change: NodeChange::RotationYaw(
                                        node_attr.alignment().as_ref().map_or(Angle::zero(), |a| a.rotation().z),
                                    ),
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        rsx! {
            div { "No node selected" }
        }
    }
}

#[component]
pub fn NodePropInput(name: String, placeholder: String, node_change: NodeChange) -> Element {
    let mut node_change_signal = use_context::<Signal<Option<NodeChange>>>();
    let input_name = format!("prop{name}");

    let (init_value, input_type, readonly) = match node_change {
        NodeChange::Name(ref node_name) => (node_name.clone(), "text", false),
        NodeChange::LIDT(lidt) => (format!("{:.2}", lidt.value / 10000.), "number", false),
        NodeChange::TranslationX(x_trans) => {
            (format!("{:.6}", x_trans.get::<meter>()), "number", false)
        }

        NodeChange::TranslationY(y_trans) => {
            (format!("{:.6}", y_trans.get::<meter>()), "number", false)
        }

        NodeChange::TranslationZ(z_trans) => {
            (format!("{:.6}", z_trans.get::<meter>()), "number", false)
        }

        NodeChange::RotationRoll(roll) => (format!("{:.6}", roll.get::<degree>()), "number", false),
        NodeChange::RotationPitch(pitch) => {
            (format!("{:.6}", pitch.get::<degree>()), "number", false)
        }

        NodeChange::RotationYaw(yaw) => (format!("{:.6}", yaw.get::<degree>()), "number", false),
        NodeChange::Inverted(inverted) => (format!("{inverted}"), "checkbox", false),
        NodeChange::NodeConst(ref val) => (val.clone(), "text", true),
        NodeChange::Property(_, _ ) => ("not used".to_owned(), "text", true)
    };

    if input_type == "checkbox" {
        rsx! {
            div { class: "form-check",
                input {
                    class: "form-check-input",
                    r#type: input_type,
                    id: input_name.clone(),
                    name: input_name.clone(),
                    value: init_value,
                    checked: true,
                                // onchange: {
                //     move |event: Event<FormData>| {
                //         if let Ok(new_distance) = event.data.parsed::<f64>() {
                //             edge.set_distance(new_distance);
                //             let edge = edge.clone();
                //             spawn(async move { graph_store.update_edge(&edge).await });
                //         }
                //     }
                // },
                }
                label { class: "form-label-check", r#for: input_name, {placeholder.clone()} }
            }
        }
    } else {
        rsx! {
            div { class: "form-floating", "data-mdb-input-init": "",
                input {
                    class: "form-control bg-dark text-light",
                    r#type: input_type,
                    id: input_name.clone(),
                    name: input_name.clone(),
                    placeholder: placeholder.clone(),
                    value: init_value,
                    "readonly": readonly,
                    onchange: {
                        move |event: Event<FormData>| {
                            match node_change {
                                NodeChange::Name(_) => {
                                    let Ok(name) = event.data.parsed::<String>();
                                    node_change_signal.set(Some(NodeChange::Name(name)));
                                }
                                NodeChange::LIDT(_) => {
                                    if let Ok(lidt) = event.data.parsed::<f64>() {
                                        node_change_signal
                                            .set(
                                                Some(
                                                    NodeChange::LIDT(
                                                        RadiantExposure::new::<joule_per_square_centimeter>(lidt),
                                                    ),
                                                ),
                                            );
                                    }
                                }
                                NodeChange::TranslationX(_) => {
                                    if let Ok(trans_x) = event.data.parsed::<f64>() {
                                        node_change_signal
                                            .set(
                                                Some(
                                                    NodeChange::TranslationX(Length::new::<meter>(trans_x)),
                                                ),
                                            );
                                    }
                                }
                                NodeChange::TranslationY(_) => {
                                    if let Ok(trans_y) = event.data.parsed::<f64>() {
                                        node_change_signal
                                            .set(
                                                Some(
                                                    NodeChange::TranslationY(Length::new::<meter>(trans_y)),
                                                ),
                                            );
                                    }
                                }
                                NodeChange::TranslationZ(_) => {
                                    if let Ok(trans_z) = event.data.parsed::<f64>() {
                                        node_change_signal
                                            .set(
                                                Some(
                                                    NodeChange::TranslationZ(Length::new::<meter>(trans_z)),
                                                ),
                                            );
                                    }
                                }
                                NodeChange::RotationRoll(_) => {
                                    if let Ok(roll) = event.data.parsed::<f64>() {
                                        node_change_signal
                                            .set(
                                                Some(NodeChange::RotationRoll(Angle::new::<degree>(roll))),
                                            );
                                    }
                                }
                                NodeChange::RotationPitch(_) => {
                                    if let Ok(pitch) = event.data.parsed::<f64>() {
                                        node_change_signal
                                            .set(
                                                Some(NodeChange::RotationPitch(Angle::new::<degree>(pitch))),
                                            );
                                    }
                                }
                                NodeChange::RotationYaw(_) => {
                                    if let Ok(yaw) = event.data.parsed::<f64>() {
                                        node_change_signal
                                            .set(
                                                Some(NodeChange::RotationYaw(Angle::new::<degree>(yaw))),
                                            );
                                    }
                                }
                                NodeChange::Inverted(_) => {
                                    if let Ok(inverted) = event.data.parsed::<bool>() {
                                        node_change_signal.set(Some(NodeChange::Inverted(inverted)));
                                    }
                                }
                                NodeChange::NodeConst(_) => {}
                                NodeChange::Property(_,_) => {}
                            };
                        }
                    },
                }
                label { class: "form-label", r#for: input_name, {placeholder.clone()} }
            }
        }
    }
}

// #[component]
// pub fn AccordionElement(number: i32, node_attr: NodeAttr) -> Element{
//     rsx!{
//         div{
//             class:"accordion accordion-borderless bg-dark ",
//             id: format!("accordionElement{number}"),
//             div{
//                 class:"accordion-item bg-dark text-light",
//                 h2{
//                     class:"accordion-header",
//                     id:format!("accordionHeading{number}"),
//                     button{
//                         class:"accordion-button collapsed bg-dark text-light",
//                         type:"button",
//                         "data-mdb-collapse-init": "",
//                         "data-mdb-target":format!("#accordionCollapse{number}"),
//                         "aria-expanded":"false",
//                         "aria-controls":format!("accordionCollapse{number}"),
//                         "General"
//                     }
//                 }
//                 div{
//                     id:format!("accordionCollapse{number}"),
//                     class:"accordion-collapse collapse  bg-dark",
//                     "aria-labelledby":format!("accordionHeading{number}"),
//                     "data-mdb-parent":format!("#accordionElement{number}"),
//                     div{
//                         class:"accordion-body  bg-dark",
//                         p{{format!("ID: {}", node_attr.uuid())}}
//                         p{ {format!("Node Type: {}", node_attr.node_type())} }
//                         p { {format!("Node Name: {}", node_attr.name())} }
//                         p { {format!("LIDT: {:4.2} J/cm²", node_attr.lidt().value/10000.)} }
//                     }
//                 }
//             }
//         }
//     }
// }
// const MAIN_CSS: Asset = asset!("./assets/main.css");
// // const PLOTLY_JS: Asset = asset!("./assets/plotly.js");
// // const THREE_MOD_JS: Asset = asset!("./assets/three_mod.js");
// // const ORBIT_CTRLS: Asset = asset!("./assets/orbitControls.js");
// const MDB_CSS: Asset = asset!("./assets/mdb.min.css");
// const MDB_JS: Asset = asset!("./assets/mdb.umd.min.js");
// const MDB_SUB_CSS: Asset = asset!("./assets/mdb_submenu.css");
// #[component]
// pub fn UserInfoWindow() -> Element {
//     let menu_item_selected = use_signal(|| None::<MenuSelection>);

// 	rsx! {
//         document::Stylesheet { href: MAIN_CSS }
//         document::Stylesheet { href: MDB_CSS }
//         document::Stylesheet { href: MDB_SUB_CSS }
//         document::Script { src: MDB_JS }
//         div { class: "d-flex flex-column text-bg-dark vh-100",
//             div {
//                 MenuBar { menu_item_selected }
//             }
//         }
// 	}
// }
