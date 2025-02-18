use std::cell::RefCell;
use std::rc::Rc;

use gloo_utils::document;
use log::debug;
use regex::Regex;
use serde_json::Value;
use uuid::Uuid;
use wasm_bindgen::prelude::{wasm_bindgen, Closure};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::{js_sys, window, CustomEvent, HtmlInputElement};
use yew::{function_component, html, use_effect, use_effect_with, use_node_ref, use_reducer, use_state, Callback, Event, FocusEvent, Html, InputEvent, KeyboardEvent, NodeRef, UseReducerHandle, UseStateHandle
};
use yew_gui::bindings::{addNode, setInverted, setLidt, setName};
use yew_gui::node_graph::callbacks::create_node_callbacks;
use yew_gui::node_graph::node_element::{Connections, HTMLNodeElement, NodeAction, NodeStates};
use yew_gui::node_graph::node_element::Node;

pub struct StateHandler {
    pub node_states: UseReducerHandle<NodeStates>,
    pub logs: UseStateHandle<Vec<String>>,
}

// Hauptkomponente für Drag-and-Drop mit Nodes und Ports
#[function_component(App)]
pub fn app() -> Html {
    let node_states = use_reducer(|| {
        NodeStates::new(
            Vec::<HTMLNodeElement>::new(),
            None,
            Connections::new(),
            None::<(Uuid, String)>,
        )
    });
    let logs = use_state(Vec::<String>::new);
    let hierarchy = use_state(|| vec!["Root".to_string()]); // Startpunkt

    let node_callbacks = create_node_callbacks(node_states.clone(), logs.clone());
    

        // Cleanup-Funktion: wird aufgerufen, wenn die Komponente entfernt wird.
        // move || {
        //     // Optional: Entfernen des Event-Listeners bei der Zerstörung der Komponente
        //     window.remove_event_listener_with_callback("open_opm_file", closure.as_ref().unchecked_ref()).unwrap();
        // }
            
        // use_effect(
        //     || {
        //         let closure = Closure::wrap(Box::new(move |event: web_sys::CustomEvent| {
        //             debug!("Event empfangen: {:?}", event);  // Debug-Ausgabe
        //             if let Some(data) = event.detail().as_string() {
        //                 debug!("Empfangenes Event-Daten: {}", data);
        //             } else {
        //                 debug!("Keine Daten im Event.");
        //             }
        //         }) as Box<dyn FnMut(_)>);

        //         // Event-Listener für "listen_event" hinzufügen
        //         window()
        //             .unwrap()
        //             .add_event_listener_with_callback("open_opm_file", closure.as_ref().unchecked_ref())
        //             .unwrap();
    
        //         // Die Closure sicher in der use_effect speichern
        //         closure.forget();
    
        //         || {} // Cleanup-Funktion, wenn die Komponente entfernt wird
        //     },
        // );
    

    // Klick auf eine Breadcrumb → Zurück zu dieser Node springen
    let navigate_to = {
        let hierarchy = hierarchy.clone();
        Callback::from(move |index: usize| {
            let mut new_hierarchy = (*hierarchy).clone();
            new_hierarchy.truncate(index + 1); // Schneidet die Hierarchie auf den gewünschten Stand ab
            hierarchy.set(new_hierarchy);
        })
    };

    let on_add_node =
        create_add_node_callback(node_states.clone(), node_callbacks.on_add_log.clone());

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
                {
                for node_states.connections().values().into_iter().map(|conn| {
                    let from_node = node_states.nodes().iter().find(|node| node.id() == conn.from).unwrap();
                    let to_node = node_states.nodes().iter().find(|node| node.id() == conn.to).unwrap();

                    let from_x = from_node.x() + 42; // Mitte des Ausgangsports
                    let from_y = from_node.y() + 100; // Unterkante des Ausgangsports
                    let to_x = to_node.x() + 42; // Mitte des Eingangsports
                    let to_y = to_node.y() - 14; // Oberkante des Eingangsports
                    let mid_x = (from_x+to_x)/2;
                    let mid_y = (from_y+to_y)/2;

                    let new_path = format!(
                        "M{},{} C{},{} {},{} {},{}",
                        from_x, from_y, from_x, from_y+60, to_x, to_y-60, to_x, to_y
                    );

                    html! {
                        <path d={new_path} stroke="black" fill="transparent" stroke-width="2"/>
                        // <text x={format!("{mid_x}")} y={format!("{mid_y}")} font-size="15" text-anchor="middle" >{"Mitte des Pfads"}</text>
                    }


                    })
                }
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
            let result = unsafe { setInverted(uuid, !node_inverted) };
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

fn change_node_lidt_state(
    node_states: UseReducerHandle<NodeStates>,
    add_log_handler: Callback<String>,
) -> Callback<()> {
    Callback::from(move |()| {
        let uuid_str = node_states
            .active_node_uuid_str()
            .unwrap_or("Unknown".to_string());
        if let Some(Ok(target)) = document()
            .get_element_by_id(format!("{}_lidtInput", uuid_str).as_str())
            .map(|e| e.dyn_into::<web_sys::HtmlInputElement>())
        {
            if let Ok(new_lidt) = target.value().parse::<f64>() {
                let node_states = node_states.clone();

                let add_log_handler = add_log_handler.clone();
                let uuid_input =
                    target.id().split("_lidtInput").collect::<Vec<&str>>()[0].to_string();

                spawn_local(async move {
                    let node_states = node_states.clone();
                    let result = unsafe { setLidt(uuid_input.clone(), new_lidt.clone()) };
                    let result = wasm_bindgen_futures::JsFuture::from(result).await;

                    match result {
                        Ok(new_js_val) => {
                            let new_json: Value =
                                serde_json::from_str(&new_js_val.as_string().unwrap()).unwrap();
                            node_states.dispatch(NodeAction::UpdateNodeLIDT(new_json.clone()));
                        }
                        Err(e) => {
                            add_log_handler.emit(format!(
                                "Error while setting node lidt. Error: {}",
                                e.as_string().unwrap()
                            ));
                        }
                    }
                });
            }
        }
    })
}

fn change_node_name_state(
    node_states: UseReducerHandle<NodeStates>,
    add_log_handler: Callback<String>,
) -> Callback<()> {
    Callback::from(move |()| {
        let uuid_str = node_states
            .active_node_uuid_str()
            .unwrap_or("Unknown".to_string());
        if let Some(Ok(target)) = document()
            .get_element_by_id(format!("{}_nameInput", uuid_str).as_str())
            .map(|e| e.dyn_into::<web_sys::HtmlInputElement>())
        {
            let new_name = target.value();
            let node_states = node_states.clone();

            let add_log_handler = add_log_handler.clone();
            let uuid_input = target.id().split("_nameInput").collect::<Vec<&str>>()[0].to_string();

            spawn_local(async move {
                let node_states = node_states.clone();
                let result = unsafe { setName(uuid_input.clone(), new_name.clone()) };
                let result = wasm_bindgen_futures::JsFuture::from(result).await;

                match result {
                    Ok(new_js_val) => {
                        let new_json: Value =
                            serde_json::from_str(&new_js_val.as_string().unwrap()).unwrap();
                        node_states.dispatch(NodeAction::UpdateNodeName(
                            uuid_input.clone(),
                            new_name.clone(),
                            new_json.clone(),
                        ));
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
        let on_name_change = change_node_name_state(node_states.clone(), add_log_handler.clone());
        let on_lidt_change = change_node_lidt_state(node_states.clone(), add_log_handler.clone());

        // Callback für Enter-Taste
        let on_keydown = {
            let on_name_change = on_name_change.clone();
            let on_lidt_change = on_lidt_change.clone();
            Callback::from(move |e: KeyboardEvent| {
                if e.key() == "Enter" {
                    document().active_element().map(|e| {
                        if e.id().contains("nameInput") {
                            on_name_change.emit(());
                        } else if e.id().contains("lidtInput") {
                            on_lidt_change.emit(());
                        } else {
                            ();
                        }
                    });
                };
            })
        };

        // Callback für Fokusverlust
        let on_blur = {
            let on_name_change = on_name_change.clone();
            let on_lidt_change = on_lidt_change.clone();
            Callback::from(move |_e: FocusEvent| {
                document().active_element().map(|e| {
                    if e.id().contains("nameInput") {
                        on_name_change.emit(());
                    } else if e.id().contains("lidtInput") {
                        on_lidt_change.emit(());
                    } else {
                        ();
                    }
                });
            })
        };

        let on_input = Callback::from(move |e: InputEvent| {
            let regex = Regex::new(r"^\d*\.\d*$").unwrap();
            let target = e.target().unwrap();
            let input: HtmlInputElement = target.unchecked_into();
            let new_value = input.value();

            if !regex.is_match(&new_value) {
                // Falls ungültig, korrigiere die Eingabe
                let mut filtered = new_value
                    .chars()
                    .filter(|c| c.is_numeric() || *c == '.')
                    .collect::<String>();

                // Stelle sicher, dass nur ein Punkt enthalten ist
                let parts: Vec<&str> = filtered.split('.').collect(); // Nur den ersten Punkt behalten
                if parts.len() > 1 {
                    filtered = format!("{}.{}", parts[0], parts[1]);
                }
                input.set_value(&filtered);
            }
        });

        html! {
        <div class="node_attributes">
            <div>
                <strong>{ "Type: " }</strong>
                { node_type }
            </div>
            <div>
                <strong>{ "Name: " }</strong>
                <input
                id={uuid.clone() + "_nameInput"}
                type="text"
                value={node_name.to_owned()}
                onkeydown={on_keydown.clone()}
                onfocusout ={on_blur.clone()}
            />
            </div>
            <div>
                <strong>{ "LIDT in J/cm²: " }</strong>
                <input
                id={uuid + "_lidtInput"}
                type="text"
                value={format!("{node_lidt}")}
                onkeydown={on_keydown}
                onfocusout ={on_blur}
                oninput = {on_input}
            />
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
            let result = unsafe { addNode(name.clone()) };
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
                            (0, 0),
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

// use gloo_utils::document;
// use wasm_bindgen::JsCast;
// use yew::prelude::*;
// use web_sys::{HtmlElement, MouseEvent};

// #[function_component(App)]
// fn app() -> Html {
//     let is_dragging = use_state(|| false);
//     let start_pos = use_state(|| None);
//     let current_pos = use_state(|| (0.0, 0.0));
//     let valid_target = use_state(|| false);
//     let connections = use_state(|| Vec::<((f64, f64), (f64, f64))>::new());

//     let on_port_mousedown = {
//         let is_dragging = is_dragging.clone();
//         let start_pos = start_pos.clone();
//         let current_pos = current_pos.clone();

//         Callback::from(move |e: MouseEvent| {
//             let x = e.client_x() as f64;
//             let y = e.client_y() as f64;

//             start_pos.set(Some((x, y)));
//             current_pos.set((x, y));
//             is_dragging.set(true);
//         })
//     };

//     let on_mouse_move = {
//         let is_dragging = is_dragging.clone();
//         let current_pos = current_pos.clone();

//         Callback::from(move |e: MouseEvent| {
//             if *is_dragging {
//                 current_pos.set((e.client_x() as f64, e.client_y() as f64));
//             }
//         })
//     };

//     let on_port_mouseup: Callback<MouseEvent> = {
//         let is_dragging = is_dragging.clone();
//         let start_pos = start_pos.clone();
//         let valid_target = valid_target.clone();
//         let connections = connections.clone();

//         Callback::from(move |e: MouseEvent| {
//             if *is_dragging && *valid_target {
//                 if let Some((sx, sy)) = *start_pos {
//                     let ex = e.client_x() as f64;
//                     let ey = e.client_y() as f64;

//                     connections.set({
//                         let mut new_connections = (*connections).clone();
//                         new_connections.push(((sx, sy), (ex, ey)));
//                         new_connections
//                     });

//                     log::info!("Verbindung gespeichert: ({}, {}) → ({}, {})", sx, sy, ex, ey);
//                 }
//             }

//             is_dragging.set(false);
//             start_pos.set(None);
//         })
//     };

//     let on_mouseup = {
//         let is_dragging = is_dragging.clone();
//         let start_pos = start_pos.clone();

//         Callback::from(move |_e: MouseEvent| {
//             is_dragging.set(false);
//             start_pos.set(None);
//         })
//     };

//     let on_svg_hover = {
//         let valid_target = valid_target.clone();
//         let is_dragging = is_dragging.clone();
//         let current_pos = current_pos.clone();
//         Callback::from(move |e: MouseEvent| {
//             if *is_dragging {
//                 let elem = e.target_dyn_into::<HtmlElement>().unwrap();

//                     let x = elem.offset_left() + 10;
//                     let y = elem.offset_top() + 10;
//                     current_pos.set((x as f64, y as f64));
//             }log::info!("valid");valid_target.set(true)}) // 🔥 Bleibt valid, wenn im SVG
//     };

//     let on_svg_leave = {
//         let valid_target = valid_target.clone();
//         Callback::from(move |_e: MouseEvent| {log::info!("invalid");valid_target.set(false)}) // 🔥 Wird nur invalid, wenn Maus das SVG verlässt
//     };

//     html! {
//     <div>
//         <div style="position:absolute; top:100px;left:100px; background-color:blue; width:20px; height:20px;"
//         onmousedown={on_port_mousedown.clone()}>
//         </div>
//         <div style="position:absolute;top:300px;left:300px; background-color:red; width:20px; height:20px;"
//         onmouseover={on_svg_hover} // 🛠 Setzt valid_target nur, wenn in SVG
//         onmouseleave={on_svg_leave} // 🛠 Setzt valid_target nur zurück, wenn SVG verlassen wird
//         onmouseup={on_port_mouseup.clone()}>
//         </div>
//         <svg
//             width="800" height="600"
//             onmousemove={on_mouse_move.clone()}
//             onmouseup={on_mouseup}
//         >
//             // <circle
//             //     cx="100" cy="100" r="10" fill="blue"
//             //     onmousedown={on_port_mousedown.clone()}
//             // />
//             // <circle
//             //     cx="300" cy="300" r="10" fill="red"
//             // onmouseover={on_svg_hover} // 🛠 Setzt valid_target nur, wenn in SVG
//             // onmouseleave={on_svg_leave} // 🛠 Setzt valid_target nur zurück, wenn SVG verlassen wird
//             //     onmouseup={on_port_mouseup.clone()}
//             // />

//             // Dynamische Verbindungslinie
//             {if *is_dragging {
//                 if let Some((sx, sy)) = *start_pos {
//                     let (cx, cy) = *current_pos;
//                     html!{

//                         <line x1={sx.to_string()} y1={sy.to_string()} x2={cx.to_string()} y2={cy.to_string()}
//                         stroke="black" stroke-width="2" />
//                     }
//                 }
//                 else{
//                     html!{}
//                 }
//             }
//             else{
//                 html!{}
//             }
//         }
//          // ✅ Alle gespeicherten Verbindungen rendern
//             { for connections.iter().map(|((sx, sy), (ex, ey))| html! {
//                 <line x1={sx.to_string()} y1={sy.to_string()} x2={ex.to_string()} y2={ey.to_string()}
//                     stroke="black" stroke-width="2" />
//             }) }
//         </svg>
//     </div>

//     }
// }
