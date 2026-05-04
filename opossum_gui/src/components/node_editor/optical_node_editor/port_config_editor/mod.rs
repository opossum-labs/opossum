#![allow(clippy::derive_partial_eq_without_eq)]
mod aperture_editor;
mod coating_editor;

use dioxus::prelude::*;
use opossum_core::{
    coatings::CoatingType,
    core_optics::optic_ports::PortConfig,
    nodes::fluence_detector::Fluence,
    prelude::{Aperture, PortType},
    types::api_types::{NodeInfo, UpdatePortRequest},
};
use uom::si::radiant_exposure::joule_per_square_centimeter;
use uuid::Uuid;

use crate::{
    OPOSSUM_UI_LOGS,
    api::{get_ports_of_group, patch_node_port_config},
    components::node_editor::{
        accordion::AccordionItem,
        inputs::input_components::{NodeConfigUnitInput, UnitHandling},
        node_config_editor::NodeChangeEvent,
        optical_node_editor::port_config_editor::{
            aperture_editor::ApertureEditor, coating_editor::CoatingEditor,
        },
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
    let mut editor_inputs = Vec::new();

    let ports_resource = use_resource(move || async move {
        match get_ports_of_group(current_node_id).await {
            Ok(ports_info) => Some(ports_info),
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                None
            }
        }
    });

    let handle_port_update = move |(p_name, p_type, req): (String, PortType, UpdatePortRequest)| {
        let n_id = *node_id.read();

        // start new asynchronous task to perform the API call, so that the UI doesn't freeze while waiting for the response.
        spawn(async move {
            if let Err(err) = patch_node_port_config(n_id, p_name, p_type, req).await {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log(&format!("API Error: {err}"));
            }
        });
    };

    if let Some(Some(ports)) = ports_resource.read_unchecked().clone() {
        for (port_name, port_config) in ports.inputs {
            editor_inputs.push(rsx!(SinglePortConfigEditor {
                node_id,
                port_name: port_name.clone(),
                port_config: port_config.clone(),
                on_change: move |req| handle_port_update((port_name.clone(), PortType::Input, req)),
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
                parent_id: "accordionNodeConfig",
                content_id: "portConfigCollapse",
                level: 1,

            }

        }
    } else {
        rsx! {
            div { "Loading port configuration..." }
        }
    }
}
#[component]
pub fn SinglePortConfigEditor(
    node_id: Memo<Uuid>,
    port_name: String,
    port_config: PortConfig,
    on_change: EventHandler<UpdatePortRequest>,
    readonly: bool,
) -> Element {
    let mut accordion_content = vec![];

    accordion_content.push(rsx! {

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
            unit_config: UnitHandling::new("J/cm²", true),
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
        ApertureEditor {
            node_id,
            aperture: port_config.aperture.clone(),
            on_change: move |aperture: Aperture| {
                let update_port_request = UpdatePortRequest {
                    aperture: Some(aperture),
                    ..Default::default()
                };
                on_change.call(update_port_request);
            },
            readonly,
        }
    });

    rsx! {
        div {
            class: "accordion accordion-borderless bg-dark border-start",
            id: "accordionPortConfig{port_name}",
            AccordionItem {
                elements: accordion_content,
                header: "Port: {port_name}",
                header_id: "singlePortHeading{port_name}",
                parent_id: "accordionPortConfig{port_name}",
                content_id: "singlePortCollapse{port_name}",
                level: 2,
            }
        }
    }
}
