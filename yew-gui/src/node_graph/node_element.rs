use web_sys::{DragEvent, HtmlElement};
use yew::{function_component, html, Callback, Html, Properties, TargetCast};

// Verbindung zwischen Ports
#[derive(Clone, PartialEq)]
pub struct Connection {
    pub from: usize,
    pub to: usize,
}

// Node-Komponente
#[derive(Properties, PartialEq)]
pub struct NodeProps {
    pub id: usize,                                // Eindeutige ID für die Node
    pub x: i32,                                   // Position der Node (x)
    pub y: i32,                                   // Position der Node (y)
    pub width: i32,                               // Breite der Node
    pub height: i32,                              // Höhe der Node
    pub on_drag_start: Callback<DragEvent>,       // Callback für den Drag-Start
    pub on_drag_end: Callback<(usize, i32, i32)>, // Callback für das Drag-Ende (ID, neue X-, Y-Position)
    pub on_port_click: Callback<(usize, String)>, // Klick auf Port (Node-ID, Port-Typ)
    pub on_node_double_click: Callback<(usize, String)>,
    pub is_active: bool,
    pub name: String,
    pub is_source: bool,
}

#[function_component(Node)]
pub fn node(props: &NodeProps) -> Html {
    let NodeProps {
        id,
        x,
        y,
        width,
        height,
        on_drag_start,
        on_drag_end,
        on_port_click,
        on_node_double_click,
        is_active,
        name,
        is_source,
    } = props;

    // Berechne die Position der Ports
    let port_w_h = 12;
    let border_radius = 2;
    let top_port_x = width / 2 - port_w_h / 2 - border_radius; // Horizontal zentriert
    let bottom_port_x = width / 2 - port_w_h / 2 - border_radius; // Horizontal zentriert

    // Position des oberen Ports
    let top_port_y = -port_w_h / 2 - 3 * border_radius / 2; // An der oberen Kante des Containers

    // Position des unteren Ports
    let bottom_port_y = height - port_w_h / 2 - border_radius / 2; // An der unteren Kante des Containers

    // Klick-Handler für Ports
    let on_input_port_click = {
        let on_port_click = on_port_click.clone();
        let id = *id;
        Callback::from(move |_| on_port_click.emit((id, "input".to_string())))
    };

    let on_output_port_click = {
        let on_port_click = on_port_click.clone();
        let id = *id;
        Callback::from(move |_| on_port_click.emit((id, "output".to_string())))
    };

    // Drag-Ende-Handler
    let on_drag_end = {
        let on_drag_end = on_drag_end.clone();
        let id = *id;
        move |event: DragEvent| {
            if let Some(target) = event.target_dyn_into::<HtmlElement>() {
                if let Some(container) = target.closest(".drop-container").unwrap() {
                    let rect = container.get_bounding_client_rect();
                    let new_x = event.page_x() as i32 - rect.left() as i32;
                    let new_y = event.page_y() as i32 - rect.top() as i32;

                    on_drag_end.emit((id, new_x, new_y)); // ID und neue Position übergeben
                }
            }
        }
    };

    let on_dblclick = {
        let on_double_click = on_node_double_click.clone();
        let id = id.clone();
        let name = name.clone();
        Callback::from(move |_| {
            on_double_click.emit((id, name.clone()));
        })
    };

    let style = if *is_active { " active-node" } else { "" };

    html! {
        <div

            ondblclick={on_dblclick}
            class={format!("node{style} draggable")}
            draggable="true"
            ondragstart={on_drag_start.clone()}
            ondragend={on_drag_end}
            style={format!("position: absolute; left: {}px; top: {}px;", x, y)}>

            <div class="node-content" style={format!("width: {}px; height: {}px;", width, height)}>{name}</div>

            // Input-Port
            <div
                class="port input-port"
                onclick={on_input_port_click}
                style={format!("left: {}px; top: {}px; width: {}px; height: {}px;", top_port_x, top_port_y, port_w_h, port_w_h)}>
            </div>
            {
            if !*is_source{
                html!{
            // Output-Port
            <div
                class="port output-port"
                onclick={on_output_port_click}
                style={format!("left: {}px; top: {}px; width: {}px; height: {}px;", bottom_port_x, bottom_port_y, port_w_h, port_w_h)}>
            </div>
            }
        }
        else{
            html!{}
        }
        }
        </div>
    }
}
