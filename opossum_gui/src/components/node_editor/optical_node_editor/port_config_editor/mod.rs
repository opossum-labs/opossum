#![allow(clippy::derive_partial_eq_without_eq)]
mod coating_editor;

use dioxus::prelude::*;
use opossum_core::{
    coatings::CoatingType,
    core_optics::optic_ports::PortConfig,
    nodes::fluence_detector::Fluence,
    prelude::PortType,
    types::api_types::{NodeInfo, UpdatePortRequest},
};
use uom::si::radiant_exposure::joule_per_square_centimeter;
use uuid::Uuid;

use crate::{
    OPOSSUM_UI_LOGS,
    api::{get_ports_of_group, patch_node_port_config},
    components::node_editor::{
        accordion::AccordionItem, inputs::input_components::NodeConfigUnitInput,
        node_config_editor::NodeChangeEvent,
        optical_node_editor::port_config_editor::coating_editor::CoatingEditor,
    },
};

#[component]
pub fn PortConfigEditor(
    node_id: Memo<Uuid>,
    node_info: ReadSignal<NodeInfo>,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let current_node_id = *node_id.read();
    let mut ports_resource =
        use_resource(move || async move { get_ports_of_group(current_node_id).await });

    let ports_data = ports_resource.read_unchecked().clone();

    ports_data.map_or_else(
        || {
            rsx! {
                div { "Loading port configuration..." }
            }
        },
        |result| match result {
            Ok(ports) => {
                let handle_port_update =
                    move |(p_name, p_type, req): (String, PortType, UpdatePortRequest)| {
                        let n_id = *node_id.read();

                        // Wir starten einen asynchronen Task
                        spawn(async move {
                            match patch_node_port_config(n_id, p_name, p_type, req).await {
                                Ok(()) => {
                                    // Nach erfolgreichem API-Call die Resource neu laden,
                                    // damit das UI die aktuellen Werte vom Server zeigt.
                                    ports_resource.restart();
                                }
                                Err(err) => {
                                    OPOSSUM_UI_LOGS
                                        .write()
                                        .add_log(&format!("API Error: {err}"));
                                }
                            }
                        });
                    };
                let mut editor_inputs = Vec::new();
                for (port_name, port_config) in ports.inputs {
                    editor_inputs.push(rsx!(SinglePortConfigEditor {
                        node_id,
                        port_name: port_name.clone(),
                        port_config: port_config.clone(),
                        on_change: move |req| handle_port_update((
                            port_name.clone(),
                            PortType::Input,
                            req
                        )),
                        readonly,
                    }));
                }
                for (port_name, port_config) in ports.outputs {
                    editor_inputs.push(rsx!(SinglePortConfigEditor {
                        node_id,
                        port_name: port_name.clone(),
                        port_config: port_config.clone(),
                        on_change: move |req| handle_port_update((
                            port_name.clone(),
                            PortType::Output,
                            req
                        )),
                        readonly,
                    }));
                }
                rsx! {
                    AccordionItem {
                        elements: editor_inputs,
                        header: "Port Configuration",
                        header_id: "portConfigHeading",
                        parent_id: "portConfigAccordion",
                        content_id: "portConfigCollapse",
                    }
                }
            }
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                rsx![]
            }
        },
    )
}
#[component]
pub fn SinglePortConfigEditor(
    node_id: Memo<Uuid>,
    port_name: String,
    port_config: PortConfig,
    on_change: EventHandler<UpdatePortRequest>,
    readonly: bool,
) -> Element {
    rsx! {
        div { class: "d-flex flex-column gap-2 border rounded p-2",
            p { class: "small", "{port_name}" }
            CoatingEditor {
                coating_type: port_config.coating.clone(),
                on_change: move |coating_type: CoatingType| {
                    let update_port_request = UpdatePortRequest {
                        coating: Some(coating_type),
                        ..Default::default()
                    };
                    on_change.call(update_port_request);
                },
                readonly,
            }
            NodeConfigUnitInput {
                id: node_id.read().to_string(),
                label: "LIDT",
                value: port_config.lidt.get().get::<joule_per_square_centimeter>(),
                base_unit: "J/cm²",
                onchange: move |value: f64| {
                    let mut update_port_request = UpdatePortRequest::default();
                    let old_fluence = port_config.lidt;
                    let mut new_fluence = old_fluence;
                    new_fluence
                        .set(Fluence::new::<joule_per_square_centimeter>(value))
                        .unwrap_or_else(|err| {
                            OPOSSUM_UI_LOGS.write().add_log(&format!("Invalid LIDT value: {err}"));
                        });
                    update_port_request.lidt = Some(new_fluence);
                    on_change.call(update_port_request);
                },
                readonly,
            }
        }
    }
}
