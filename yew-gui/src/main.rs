
use gloo_utils::document;
use serde_json::Value;
use wasm_bindgen::JsCast;
use yew_gui::node_graph::callbacks::create_node_callbacks;
use std::collections::HashMap;
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;
use yew::{
    function_component, html, use_node_ref, use_reducer, use_state, Callback, Event, FocusEvent, Html, KeyboardEvent, NodeRef, UseReducerHandle, UseStateHandle
};
use yew_gui::bindings::{addNode, setInverted};
use yew_gui::node_graph::node_element::{Connections, HTMLNodeElement, NodeAction, NodeStates};
use yew_gui::{
    bindings::setName,
    node_graph::node_element::{Connection, Node},
};
use log::debug;


pub struct StateHandler{
    pub node_states: UseReducerHandle<NodeStates>,
    pub logs: UseStateHandle<Vec<String>>,
}


// Hauptkomponente für Drag-and-Drop mit Nodes und Ports
#[function_component(App)]
pub fn app() -> Html {
    let node_states = use_reducer( || NodeStates::new(Vec::<HTMLNodeElement>::new(), None, Connections::new(), None::<(Uuid, String)>));
    let logs = use_state(Vec::<String>::new);
    let hierarchy = use_state(|| vec!["Root".to_string()]); // Startpunkt

    let node_callbacks = create_node_callbacks(
        node_states.clone(),
        logs.clone(),
    );

    // Klick auf eine Breadcrumb → Zurück zu dieser Node springen
    let navigate_to = {
        let hierarchy = hierarchy.clone();
        Callback::from(move |index: usize| {
            let mut new_hierarchy = (*hierarchy).clone();
            new_hierarchy.truncate(index + 1); // Schneidet die Hierarchie auf den gewünschten Stand ab
            hierarchy.set(new_hierarchy);
        })
    };

    let on_add_node = create_add_node_callback(node_states.clone(), node_callbacks.on_add_log.clone());


    let avail_nodes = available_nodes().clone();
    html! {
        <div class="container">
            <div class="menu">
                <div class="dropdown">
                <button class="dropbtn">{"Add Nodes"}</button>
                <div class="dropdown-content">
                {
                    for avail_nodes.into_iter().map(|n| {
                        let n_name = n.clone();
                        if n == "source"{
                            html!{
                                <a href="#" onclick={on_add_node.reform(move |_| (n.to_string(), true))}>{n_name}</a>
                            }
                        }
                        else{
                            html!{
                                <a href="#" onclick={on_add_node.reform(move |_| (n.to_string(), false))}>{n_name}</a>
                            }
                        }
                    })
                }
                </div>
                </div>
                <div class="NodeAttr" style="margin-top: 20px;">
                    {extract_node_info(node_states.clone(), node_callbacks.on_add_log.clone())}
                </div>
            </div>

            <div class="main-window">
                <div class="graph-hierarchy">
                        { for hierarchy.iter().enumerate().map(|(index, name)| {
                            let navigate_to = navigate_to.clone();
                            let name_clone = name.clone();
                            html! {
                                <span
                                    onclick={Callback::from(move |_| navigate_to.emit(index))}
                                    style="cursor: pointer; color: blue; text-decoration: underline;"
                                >
                                    { name_clone } { " / " }
                                </span>
                            }
                        }) }
                    </div>
            <div class="drop-container">
                // Verbindungen als Linien rendern
                <svg class="connections" style="position: absolute; width: 100%; height: 100%;">
                    { for node_states.connections().values().into_iter().map(|conn| {
                        let from_node = node_states.nodes().iter().find(|node| node.id() == conn.from).unwrap();
                        let to_node = node_states.nodes().iter().find(|node| node.id() == conn.to).unwrap();

                        let from_x = from_node.x() + 42; // Mitte des Ausgangsports
                        let from_y = from_node.y() + 100; // Unterkante des Ausgangsports
                        let to_x = to_node.x() + 42; // Mitte des Eingangsports
                        let to_y = to_node.y() - 14; // Oberkante des Eingangsports

                        let new_path = format!(
                            "M{},{} C{},{} {},{} {},{}",
                            from_x, from_y, from_x, from_y+60, to_x, to_y-60, to_x, to_y
                        );

                        html! {
                        <path d={new_path} stroke="black" fill="transparent" stroke-width="2"/>
                        }
                    }) }
                </svg>
                { for node_states.nodes().iter().map(|node| {
                    let is_active = if let Some(json_val) = node_states.active_node().clone(){
                        let id_active = Uuid::parse_str(&json_val["uuid"].as_str().unwrap()).unwrap();
                        node.id() == id_active
                    }
                    else{
                        false
                    };
                html! {
                    <Node
                        html_node={node.clone()}
                        width={100}
                        height={100}
                        node_callbacks={node_callbacks.clone()}
                        is_active = {is_active.clone()}
                    />
                }
                }) }
            </div>
            <div>
            <div class="log-container" id="log-container">
                { for logs.iter().map(|log| html! { <p class="log-entry">{ log }</p> }) }
            </div>
        </div>
            </div>
        </div>
    }
}

fn change_node_inverted_state(
    node_states: UseReducerHandle<NodeStates>,
    add_log_handler: Callback<String>,
    node_inverted: bool,
    uuid: String,
) -> Callback<Event> {
    Callback::from(move |_| {
        
        let node_states = node_states.clone();
        let add_log_handler = add_log_handler.clone();
        let uuid = uuid.clone();
        spawn_local(async move {
            let node_states = node_states.clone();
            let result = setInverted(uuid, !node_inverted);
            let result = wasm_bindgen_futures::JsFuture::from(result).await;

            match result {
                Ok(new_js_val) => {
                    let new_json: Value =
                        serde_json::from_str(&new_js_val.as_string().unwrap()).unwrap();
                        add_log_handler.emit(format!(
                        "Changed inverted status of node \"{}\" to {}.",
                        new_json["name"].as_str().unwrap_or("Unknown"),
                        new_json["inverted"].to_string()
                    ));
                    node_states.dispatch(NodeAction::NodeDoubleClick(new_json));
                }
                Err(e) => {
                    add_log_handler.emit(format!(
                        "Error while inverting node. Error: {}",
                        e.as_string().unwrap()
                    ));
                }
            }
        });
    })
}

fn change_node_name_state(
    node_states: UseReducerHandle<NodeStates>,
    add_log_handler: Callback<String>,
) -> Callback<()> {
    Callback::from(move |()| {
        let uuid_str = node_states.active_node_uuid_str().unwrap_or("Unknown".to_string());
        if let Some(Ok(target)) = document().get_element_by_id(format!("{}_nameInput", uuid_str).as_str()).map(|e| e.dyn_into::<web_sys::HtmlInputElement>()) {   
            let new_name = target.value();
            let node_states = node_states.clone();

            let add_log_handler = add_log_handler.clone();
            let uuid_input = target.id().split("_nameInput").collect::<Vec<&str>>()[0].to_string();

            spawn_local(async move {
                let node_states = node_states.clone();
                let result = setName(uuid_input.clone(), new_name.clone());
                let result = wasm_bindgen_futures::JsFuture::from(result).await;

                match result {
                    Ok(new_js_val) => {
                        let new_json: Value =
                            serde_json::from_str(&new_js_val.as_string().unwrap()).unwrap();
                            node_states.dispatch(NodeAction::UpdateNodeName(uuid_input.clone(), new_name.clone(), new_json.clone()));
                    }
                    Err(e) => {
                        add_log_handler.emit(format!(
                            "Error while setting node name. Error: {}",
                            e.as_string().unwrap()
                        ));
                    }
                }
            });
        }
    })
}

fn extract_node_info(
    node_states: UseReducerHandle<NodeStates>,
    add_log_handler: Callback<String>,
) -> Html {
    if let Some(json_val) = (*node_states).active_node().clone() {
        let node_inverted = json_val["inverted"].as_bool().unwrap_or(false);
        let uuid = json_val["uuid"].as_str().unwrap_or("Unknown").to_owned();
        let node_type = json_val["node_type"].as_str().unwrap_or("Unknown");
        let node_name = json_val["name"].as_str().unwrap_or("Unknown");
        let node_lidt = json_val["lidt"].as_f64().unwrap_or(0.0) / 10000.;

        let on_inverted_change = change_node_inverted_state(
            node_states.clone(),
            add_log_handler.clone(),
            node_inverted,
            uuid.clone(),
        );
        let on_name_change = change_node_name_state(
            node_states.clone(),
            add_log_handler.clone(),
        );

        // Callback für Enter-Taste
        let on_keydown = {
            let on_name_change = on_name_change.clone();
            Callback::from(move |e: KeyboardEvent| {
                if e.key() == "Enter" {
                    on_name_change.emit(());
                }
            })
        };

        // Callback für Fokusverlust
        let on_blur = {
            let on_name_change = on_name_change.clone();
            Callback::from(move |_e: FocusEvent| {
                on_name_change.emit(());
            })
        };

        html! {
        <div class="node_attributes">
            <div>
                <strong>{ "Node Type: " }</strong>
                { node_type }
            </div>
            <div>
                <strong>{ "Node Name: " }</strong>
                <input
                id={uuid + "_nameInput"}
                type="text"
                value={node_name.to_owned()}  // Den aktuellen Namen anzeigen
                onkeydown={on_keydown}  // Übernimmt den Namen bei Drücken der Enter-Taste
                onfocusout ={on_blur}        // Übernimmt den Namen bei Fokusverlust
            />
            </div>
            <div>
                <strong>{ "Node LIDT: " }</strong>
                { format!("{} J/cm²", node_lidt) }
            </div>
            <div>
                <strong>{ "Node Inverted: " }</strong>
                <input type="checkbox" checked={node_inverted} onchange={on_inverted_change}/>
            </div>
        </div>
        }
    } else {
        html! {
            <div>
                <strong>{ "No node selected" }</strong>
            </div>
        }
    }
}
fn main() {
    console_log::init_with_level(log::Level::Debug).expect("error initializing log");
    yew::Renderer::<App>::new().render();
    debug!("Yew App gestartet!");
}

fn available_nodes() -> Vec<String> {
    vec![
        "dummy".to_string(),
        "beam splitter".to_string(),
        "energy meter".to_string(),
        "group".to_string(),
        "ideal filter".to_string(),
        "reflective grating".to_string(),
        "reference".to_string(),
        "lens".to_string(),
        "cylindric lens".to_string(),
        "source".to_string(),
        "spectrometer".to_string(),
        "spot diagram".to_string(),
        "wavefront monitor".to_string(),
        "paraxial surface".to_string(),
        "ray propagation".to_string(),
        "fluence detector".to_string(),
        "wedge".to_string(),
        "mirror".to_string(),
        "parabolic mirror".to_string(),
    ]
}

// Die Funktion, die den Callback zurückgibt
fn create_add_node_callback(
    node_states: UseReducerHandle<NodeStates>,
    add_log_handler: Callback<String>,
) -> Callback<(String, bool)> {
    Callback::from(move |(name, is_source): (String, bool)| {
        let node_states = node_states.clone();
        let add_log_handler = add_log_handler.clone();
        
        // Erzeuge eine zufällige Position
        let new_x = 50 + (node_states.nodes().len() as i32 % 20) * 100;
        let new_y = 50 + (node_states.nodes().len() as i32 / 20) * 100;
        let mut new_nodes = (*node_states).nodes().clone();
        spawn_local(async move {
            let result = addNode(name.clone());
            let result = wasm_bindgen_futures::JsFuture::from(result).await;
            match result {
                Ok(new_id) => {
                    if let Ok(new_uuid) = Uuid::parse_str(&new_id.as_string().unwrap()) {
                        let new_node = HTMLNodeElement::new(
                            new_uuid,
                            new_x,
                            new_y,
                            name.clone(),
                            is_source.clone(),
                            (0,0)
                        );
                        new_nodes.push(new_node.clone());
                        node_states.dispatch(NodeAction::AddNode(new_node));
                    } else {
                        add_log_handler.emit(format!(
                            "Error while adding node \"{}\" due to uuid deserialization failure",
                            name
                        ));
                    }
                }
                Err(e) => {
                    add_log_handler.emit(format!(
                        "Error while adding node \"{}\". Error: {}",
                        name,
                        e.as_string().unwrap()
                    ));
                }
                _ => {
                    add_log_handler.emit(format!("Error: unknown"));
                }
            }
        });
    })
}

