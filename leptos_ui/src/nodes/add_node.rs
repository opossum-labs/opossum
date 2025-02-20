use std::sync::Arc;

use leptos::{
    component,
    prelude::{
        expect_context, use_context, ArcRwSignal, ClassAttribute, ElementChild, For, Get,
        OnAttribute, Read, ReadSignal, RwSignal, Write, WriteSignal,
    },
    reactive::signal,
    view, IntoView,
};
use log::debug;
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    nodes::node_element::{Node, NodesStore, NodesStoreStoreFields},
    HTMLNodeElement,
};

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

#[component]
pub fn AddNodeDropDown() -> impl IntoView {
    let nodes = expect_context::<Store<NodesStore>>();
    // use_context::<RwSignal<Vec<RwSignal<HTMLNodeElement>>>>().expect("no context for RwSignal<Vec<HTMLNodeElement>> found");
    let avail_nodes = available_nodes().clone();
    view! {
        <div class="dropdown">
            <button class="dropbtn">{"Add Nodes"}</button>
            <div class="dropdown-content">
            {
                move || avail_nodes.iter().map(|n| {
                    let name = n.clone();
                    let nodes = nodes.nodes();
                    let nr_of_nodes = nodes.get().len();
                    let new_x = 50 + (nr_of_nodes as i32 % 20) * 100;
                    let new_y = 50 + (nr_of_nodes as i32 / 20) * 100;
                    let new_node = HTMLNodeElement::new(
                        new_x,
                        new_y,
                        Uuid::new_v4(),
                        name.clone(),
                        n == "source",
                        (0, 0),
                    );
                    view! {
                        <a href="#" on:click=move |_| {
                            nodes.write().push(new_node.clone())
                        }  >{name}</a>

                    }
                }).collect::<Vec<_>>()
            }
            </div>
        </div>
    }
}

#[component]
pub fn Nodes() -> impl IntoView {
    let nodes = expect_context::<Store<NodesStore>>();
    view! {
        <For
            each = move || nodes.nodes().get()
            key = |node| node.id().clone()
            let(node)
        >
            <Node node = node />
        </For>
    }
}
