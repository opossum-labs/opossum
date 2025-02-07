use std::collections::HashMap;
use web_sys::{DragEvent, HtmlElement};
use yew::{function_component, html, use_state, Callback, Html, TargetCast, UseStateHandle};
use yew_gui::node_graph::node_element::{Connection, Node};

#[cfg(not(target_arch = "wasm32"))]
use opossum::nodes;

// Hauptkomponente für Drag-and-Drop mit Nodes und Ports
#[function_component(App)]
pub fn app() -> Html {
    let nodes = use_state(|| Vec::<(usize, i32, i32, String, bool)>::new());
    let connections = use_state(|| HashMap::<usize, Connection>::new()); // Verbindungen
    let selected_port = use_state(|| None::<(usize, String)>); // Aktuell ausgewählter Port (Node-ID, Port-Typ)
    let active_node = use_state(|| None::<(usize, String)>); // Aktuell ausgewählter Port (Node-ID, Port-Typ)
    let hierarchy = use_state(|| vec!["Root".to_string()]); // Startpunkt
                                                            // Maus-Offset innerhalb der Node (x, y)
    let offset = use_state(|| (0, 0));

    // Klick auf eine Breadcrumb → Zurück zu dieser Node springen
    let navigate_to = {
        let hierarchy = hierarchy.clone();
        Callback::from(move |index: usize| {
            let mut new_hierarchy = (*hierarchy).clone();
            new_hierarchy.truncate(index + 1); // Schneidet die Hierarchie auf den gewünschten Stand ab
            hierarchy.set(new_hierarchy);
        })
    };

    let on_add_node = create_add_node_handler(nodes.clone());

    let on_node_double_click = {
        let active_node = active_node.clone();
        Callback::from(move |(id, name): (usize, String)| {
            active_node.set(Some((id, name)));
        })
    };

    let on_port_click = create_on_port_click_handler(selected_port.clone(), connections.clone());

    // Start-Handler für Drag
    let drag_start = {
        let offset = offset.clone();

        Callback::from(move |event: DragEvent| {
            if let Some(target) = event.target_dyn_into::<HtmlElement>() {
                let rect = target.get_bounding_client_rect();
                let offset_x = event.page_x() as i32 - rect.left() as i32;
                let offset_y = event.page_y() as i32 - rect.top() as i32;

                offset.set((offset_x, offset_y));
            }
        })
    };

    // Handler für das Drag-Ende (Position der Node aktualisieren)
    let on_drag_end = {
        let nodes = nodes.clone();
        let offset = offset.clone();
        Callback::from(move |(id, x, y): (usize, i32, i32)| {
            let (offset_x, offset_y) = *offset;
            let mut updated_nodes = (*nodes).clone();
            if let Some(node) = updated_nodes
                .iter_mut()
                .find(|(node_id, _, _, _, _)| *node_id == id)
            {
                node.1 = x - offset_x; // X-Position mit Offset aktualisieren
                node.2 = y - offset_y; // Y-Position mit Offset aktualisieren
            }
            nodes.set(updated_nodes); // Aktualisiere den Zustand
        })
    };

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
                    <div>
                        <strong>{ "Selected node: " }</strong>
                        { match &*active_node {
                            Some((_, name)) => name.clone(),
                            None => "None".to_string(),
                        }}
                    </div>
                    // <div>
                    //     node_type: String,
                    //     name: String,
                    //     ports: OpticPorts,
                    //     uuid: Uuid,
                    //     lidt: Fluence,
                    //     #[serde(default)]
                    //     props: Properties,
                    //     #[serde(skip_serializing_if = "Option::is_none")]
                    //     isometry: Option<Isometry>,
                    //     #[serde(default)]
                    //     inverted: bool,
                    // </div>
                </div>
            </div>

            <div style = "width: 100%; height: 100% ">
                <div style="padding: 10px; display: flex; gap: 10px; ">
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
                    { for connections.values().into_iter().map(|conn| {
                        let from_node = nodes.iter().find(|(id, _, _,_,_)| *id == conn.from).unwrap();
                        let to_node = nodes.iter().find(|(id, _, _,_,_)| *id == conn.to).unwrap();

                        let from_x = from_node.1 + 42; // Mitte des Ausgangsports
                        let from_y = from_node.2 + 100; // Unterkante des Ausgangsports
                        let to_x = to_node.1 + 42; // Mitte des Eingangsports
                        let to_y = to_node.2 - 14; // Oberkante des Eingangsports

                        let new_path = format!(
                            "M{},{} C{},{} {},{} {},{}",
                            from_x, from_y, from_x, from_y+60, to_x, to_y-60, to_x, to_y
                        );

                        html! {
                        <path d={new_path} stroke="black" fill="transparent" stroke-width="2"/>
                        }
                    }) }
                </svg>
                { for nodes.iter().map(|(id, x, y, name, is_source)| {
                    let is_active = if let Some((id_active, _)) = *active_node{
                        *id == id_active
                    }
                    else{
                        false
                    };
                html! {
                    <Node
                        id={*id}
                        x={*x}
                        y={*y}
                        width={100}
                        height={100}
                        on_drag_start={drag_start.clone()}
                        on_drag_end={on_drag_end.clone()}
                        on_port_click={on_port_click.clone()}
                        on_node_double_click = {on_node_double_click.clone()}
                        is_active = {is_active.clone()}
                        name = {name.clone()}
                        is_source = {is_source.clone()}
                    />
                }
                }) }
            </div>
            </div>
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
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
fn create_add_node_handler(
    nodes: UseStateHandle<Vec<(usize, i32, i32, String, bool)>>,
) -> Callback<(String, bool)> {
    Callback::from(move |(name, is_source): (String, bool)| {
        // Erzeuge eine zufällige Position
        let new_id = nodes.len();
        let new_x = 50 + (new_id as i32 % 20) * 100;
        let new_y = 50 + (new_id as i32 / 20) * 100;

        let mut new_nodes = (*nodes).clone();
        new_nodes.push((new_id, new_x, new_y, name.clone(), is_source.clone())); // Neue Position für die Node
        nodes.set(new_nodes);
    })
}

fn create_on_port_click_handler(
    port: UseStateHandle<Option<(usize, String)>>,
    connections: UseStateHandle<HashMap<usize, Connection>>,
) -> Callback<(usize, String)> {
    Callback::from(move |(node_id, port_type): (usize, String)| {
        if let Some((selected_id, selected_type)) = (*port).clone() {
            if selected_type != port_type && selected_id != node_id {
                let to_id = if selected_type == "input" {
                    selected_id
                } else {
                    node_id
                };
                let mut conns = (*connections).clone();
                if conns
                    .values()
                    .into_iter()
                    .fold(true, |arg0, c| (c.to != to_id) & arg0)
                {
                    // Verbindung erstellen
                    connections.set({
                        conns.insert(
                            if selected_type == "output" {
                                selected_id
                            } else {
                                node_id
                            },
                            Connection {
                                from: if selected_type == "output" {
                                    selected_id
                                } else {
                                    node_id
                                },
                                to: if selected_type == "input" {
                                    selected_id
                                } else {
                                    node_id
                                },
                            },
                        );

                        conns
                    });
                }
                port.set(None); // Auswahl zurücksetzen
            }
        } else {
            // Aktuellen Port auswählen
            port.set(Some((node_id, port_type)));
        }
    })
}
