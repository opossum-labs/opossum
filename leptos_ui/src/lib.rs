pub mod nodes;
use std::{rc::Rc, sync::Arc};

use leptos::{
    component,
    prelude::{
        provide_context, signal, use_context, ClassAttribute, ElementChild, Get, GlobalAttributes,
        IntoAny, OnAttribute, RwSignal, Set, Write,
    },
    task::spawn_local,
    view, IntoView,
};
use nodes::{
    add_node::{AddNodeDropDown, Nodes},
    node_element::{HTMLNodeElement, Node, NodesStore},
};
use reactive_stores::Store;

#[component]
pub fn App() -> impl IntoView {
    let logs = RwSignal::new(Vec::<String>::new());
    // let nodes_store: RwSignal<Vec<RwSignal<HTMLNodeElement>>> = RwSignal::new(Vec::<RwSignal<HTMLNodeElement>>::new());
    let nodes_store = Store::new(NodesStore::default());
    provide_context(logs);
    provide_context(nodes_store);

    // let on_add_node = create_add_node_callback(nodes);
    view! {
        <div class="container">
            <div class="menu">
                <AddNodeDropDown/>
            <button
            on:click=move |_| {
                logs.write().push("pressed!".to_owned())
            }
                >
                    "add log: "
                </button>
            </div>

            <div class="main-window">
            <div class="drop-container">
            <Nodes />
            </div>
                <Logger />
            </div>
        </div>
    }
}

#[component]
fn Logger() -> impl IntoView {
    let log_signal = use_context::<RwSignal<Vec<String>>>()
        .expect("could not find RwSignal<Vec<String>> in context");
    view! {<div>
            <div class="log-container" id="log-container">
                {
                    move || log_signal.read_only().get().iter().map(|log| {
                        view! { <p class="log-entry">{ format!("{log}") }</p> }
                    }).collect::<Vec<_>>()
                }

            </div>
        </div>
    }
}

// // Die Funktion, die den Callback zurückgibt
// fn create_add_node_callback(
//     nodes: RwSignal<Vec<HTMLNodeElement>>,
// ) -> impl Fn((String, bool)) -> Vec<impl IntoView> {
//     let test  = move |(name, is_source): (String, bool)| {
//         let new_x = 50 + (nodes.get().len() as i32 % 20) * 100;
//         let new_y = 50 + (nodes.get().len() as i32 / 20) * 100;
//         let new_node = HTMLNodeElement::new(
//             new_x,
//             new_y,
//             name.clone(),
//             is_source.clone(),
//             (0, 0),
//         );
//         let mut nodesss = nodes.get();
//         nodesss.push(new_node);
//         nodes.set(nodesss);
//         nodesss.iter().map(|n| PeterPan(node_element::PeterPanProps {html_node:n.clone()})).collect::<Vec<_>>()
//     };
//     test
// }
