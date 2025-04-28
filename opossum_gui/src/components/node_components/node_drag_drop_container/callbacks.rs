use std::rc::Rc;

use super::{node_editor::ZoomShift, DraggedNode, NodeOffset};
use crate::{
    api::{self},
    components::node_components::edges::edges_component::EdgeCreation,
    EDGES, HTTP_API_CLIENT, NODES_STORE, OPOSSUM_UI_LOGS, ZOOM,
};
use dioxus::prelude::*;

pub fn use_on_mouse_move() -> impl FnMut(Event<MouseData>) {
    let offset = use_context::<Signal<NodeOffset>>();
    let dragged_node = use_context::<Signal<DraggedNode>>();
    let mut edge_in_creation = use_context::<Signal<Option<EdgeCreation>>>();

    move |event: MouseEvent| {
        if let (Some(id), Some(elem_offset)) = (
            dragged_node.read().node_id(),
            dragged_node.read().elem_offset(),
        ) {
            NODES_STORE.write().drag_node(id, elem_offset, &event.data);
        }
        if let (Some(edge_creation), Some(offset)) =
            (edge_in_creation.write().as_mut(), offset.read().offset())
        {
            edge_creation.set_end_x(event.client_coordinates().x - offset.0);
            edge_creation.set_end_y(event.client_coordinates().y - offset.1);
        };
    }
}

pub fn use_on_wheel() -> impl FnMut(Event<WheelData>) {
    let offset = use_context::<Signal<NodeOffset>>();
    let mut edge_in_creation = use_context::<Signal<Option<EdgeCreation>>>();

    move |event: WheelEvent| {
        ZOOM.write().set_zoom_from_scroll_event(&event);
        let zoom_factor = ZOOM.read().zoom_factor();
        let offset = offset.read();
        if let Some(rect) = offset.offset() {
            let mouse_x = event.data.page_coordinates().x - rect.0;
            let mouse_y = event.data.page_coordinates().y - rect.1;

            NODES_STORE
                .write()
                .zoom_shift(zoom_factor, (mouse_x, mouse_y), (mouse_x, mouse_y));
            EDGES
                .write()
                .zoom_shift(zoom_factor, (mouse_x, mouse_y), (mouse_x, mouse_y));
            if let Some(edge_in_creation) = edge_in_creation.write().as_mut() {
                edge_in_creation.zoom_shift(zoom_factor, (mouse_x, mouse_y), (mouse_x, mouse_y));
            }
        }
    }
}

pub fn use_on_resize() -> impl FnMut(Event<ResizeData>) {
    let drop_area = use_context::<Signal<Option<Rc<MountedData>>>>();
    let mut offset = use_context::<Signal<NodeOffset>>();

    move |_: Event<ResizeData>| {
        spawn(async move {
            if let Some(drop_area) = drop_area() {
                if let Ok(rect) = drop_area.get_client_rect().await {
                    offset.write().set_offset(Some((
                        rect.origin.x,
                        rect.origin.y,
                        rect.size.width,
                        rect.size.height,
                    )));
                }
            }
        });
    }
}

pub fn use_on_mounted() -> impl FnMut(Event<MountedData>) {
    let mut drop_area = use_context::<Signal<Option<Rc<MountedData>>>>();

    move |e: Event<MountedData>| drop_area.set(Some(e.data()))
}

pub fn use_on_mouse_up() -> impl FnMut(MouseEvent) {
    let mut dragged_node = use_context::<Signal<DraggedNode>>();
    let mut edge_in_creation = use_context::<Signal<Option<EdgeCreation>>>();

    move |_: MouseEvent| {
        dragged_node.write().clear();
        edge_in_creation.set(None);
    }
}

pub fn use_on_key_down() -> impl FnMut(Event<KeyboardData>) {
    move |e: Event<KeyboardData>| {
        if e.data().key() == Key::Delete {
            if let Some(active_node_id) = NODES_STORE.read().get_active_node_id() {
                spawn(async move {
                    match api::delete_node(&HTTP_API_CLIENT(), active_node_id).await {
                        Ok(id_vec) => {
                            for id in &id_vec {
                                NODES_STORE.write().delete_node(*id);
                            }
                        }
                        Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                    }
                });
            }
        }
    }
}

pub fn use_on_double_click() -> impl FnMut(Event<MouseData>) {
    let offset = use_context::<Signal<NodeOffset>>();
    move |_: Event<MouseData>| {
        let mut zoom = ZOOM.write();
        let mut new_zoom = zoom.current();
        if NODES_STORE.read().nr_of_optic_nodes() > 1 {
            let (min_x, min_y, max_y, max_x) = NODES_STORE.read().get_min_max_position();
            let (center_x, center_y) = ((max_x + min_x) / 2., (max_y + min_y) / 2.);
            let min_dist = 150. * new_zoom;
            if let Some((_, _, width, height)) = offset.read().offset() {
                let max_zoom =
                    ((max_x - min_x + min_dist) / width).max((max_y - min_y + min_dist) / height);
                new_zoom /= max_zoom;
                zoom.set_current(new_zoom);

                //zoom to fit nodes around center of container
                NODES_STORE.write().zoom_shift(
                    zoom.zoom_factor(),
                    (width / 2., height / 2.),
                    (center_x, center_y),
                );

                EDGES.write().zoom_shift(
                    zoom.zoom_factor(),
                    (width / 2., height / 2.),
                    (center_x, center_y),
                );
            }
        } else {
            zoom.set_current(1.);
        }
    }
}
