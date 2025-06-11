use crate::components::node_editor::lens_editor::LensProperties;
use crate::components::node_editor::{alignment_editor::AlignmentEditor,general_editor::GeneralEditor
,lens_editor::LensEditor
,source_editor::SourceEditor};
use crate::{api, components::scenery_editor::node::NodeElement, HTTP_API_CLIENT, OPOSSUM_UI_LOGS};
use dioxus::prelude::*;
use opossum_backend::{
    millimeter, Fluence, Isometry, NodeAttr, Proptype,
    RefractiveIndexType,
light_data_builder::LightDataBuilder,
};
use serde_json::Value;
use uom::si::{f64::{Angle, RadiantExposure}, {angle::degree, f64::Length, length::meter}, radiant_exposure::joule_per_square_centimeter};

use super::source_editor::LightDataBuilderHistory;

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
    NodeConst(String), 
    Property(String, Value),
    Isometry(Isometry), 
}

fn extract_light_data_info(node_attr: &NodeAttr) -> (LightDataBuilder, &'static str) {
    match node_attr.properties().get("light data") {
        Ok(Proptype::LightDataBuilder(Some(ld)))
            if matches!(ld, LightDataBuilder::Geometric(_)) =>
        {
            (ld.clone(), "Rays")
        }
        Ok(Proptype::LightDataBuilder(Some(ld))) => (ld.clone(), "Energy"),
        _ => (LightDataBuilder::default(), "Rays"),
    }
}


#[component]
pub fn NodeEditor(mut node: Signal<Option<NodeElement>>) -> Element {
    let node_change = use_context_provider(|| Signal::new(None::<NodeChange>));
    let mut light_data_builder_hist = LightDataBuilderHistory::default();
    let LensProperties = use_context_provider(|| {
        Signal::new((
            millimeter!(500.),
            millimeter!(-500.),
            millimeter!(10.),
            1.5,
        ))
    });
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
                NodeChange::Isometry(iso) => {
                    spawn(async move {
                        if let Err(err_str) =
                            api::update_node_isometry(&HTTP_API_CLIENT(), active_node.id(), iso)
                                .await
                        {
                            OPOSSUM_UI_LOGS.write().add_log(&err_str);
                        };
                    });
                }
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
        if node_attr.node_type() == "source" {
            let (ld_builder, key) = extract_light_data_info(&node_attr);
            light_data_builder_hist.replace_or_insert_and_set_current(key, ld_builder)
        }



        rsx! {
            div {
                h6 { "Node Configuration" }
                div {
                    class: "accordion accordion-borderless bg-dark ",
                    id: "accordionNodeConfig",
                    GeneralEditor {
                        node_id: node_attr.uuid(),
                        node_type: node_attr.node_type(),
                        node_name: node_attr.name(),
                        node_lidt: node_attr.lidt().clone(),
                    }
                    SourceEditor {
                        hidden: node_attr.node_type() != "source",
                        light_data_builder_hist,
                        node_change,
                    }
                    LensEditor {
                        hidden: node_attr.node_type() != "lens",
                        node_change,
                        lens_properties: LensProperties::from(node_attr),
                    }
                    AlignmentEditor { alignment: node_attr.alignment().clone() }
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
        NodeChange::Property(_, _) => ("not used".to_owned(), "text", true),
        NodeChange::Isometry(_) => ("not used".to_owned(), "text", true),
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
            div {
                class: "form-floating border-start",
                "data-mdb-input-init": "",
                input {
                    class: "form-control bg-dark text-light form-control-sm",
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
                                NodeChange::Property(_, _) => {}
                                NodeChange::Isometry(_) => {}
                            };
                        }
                    },
                }
                label { class: "form-label text-secondary", r#for: input_name, {placeholder.clone()} }
            }
        }
    }
}
