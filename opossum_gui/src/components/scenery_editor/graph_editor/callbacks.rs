use crate::{
    api::{self},
    components::scenery_editor::{edges::edges_component::EdgeCreation, EDGES},
    HTTP_API_CLIENT, OPOSSUM_UI_LOGS,
};
use dioxus::prelude::*;
use std::rc::Rc;

// pub fn use_on_resize() -> impl FnMut(Event<ResizeData>) {
//     let drop_area = use_context::<Signal<Option<Rc<MountedData>>>>();
//     let mut offset = use_context::<Signal<NodeOffset>>();

//     move |_: Event<ResizeData>| {
//         spawn(async move {
//             if let Some(drop_area) = drop_area() {
//                 if let Ok(rect) = drop_area.get_client_rect().await {
//                     offset.write().set_offset(Some((
//                         rect.origin.x,
//                         rect.origin.y,
//                         rect.size.width,
//                         rect.size.height,
//                     )));
//                 }
//             }
//         });
//     }
// }

pub fn use_on_mounted() -> impl FnMut(Event<MountedData>) {
    let mut drop_area = use_context::<Signal<Option<Rc<MountedData>>>>();

    move |e: Event<MountedData>| drop_area.set(Some(e.data()))
}

// pub fn use_on_key_down() -> impl FnMut(Event<KeyboardData>) {
//     move |e: Event<KeyboardData>| {
//         if e.data().key() == Key::Delete {
//             if let Some(active_node_id) = NODES_STORE.read().get_active_node_id() {
//                 spawn(async move {
//                     match api::delete_node(&HTTP_API_CLIENT(), active_node_id).await {
//                         Ok(id_vec) => {
//                             for id in &id_vec {
//                                 NODES_STORE.write().delete_node(*id);
//                             }
//                         }
//                         Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
//                     }
//                 });
//             }
//         }
//     }
// }

// pub fn use_on_double_click() -> impl FnMut(Event<MouseData>) {
//     let offset = use_context::<Signal<NodeOffset>>();
//     move |_: Event<MouseData>| {
//         let mut zoom = ZOOM.write();
//         let mut new_zoom = zoom.current();
//         if NODES_STORE.read().nr_of_optic_nodes() > 1 {
//             let (min_x, min_y, max_y, max_x) = NODES_STORE.read().get_min_max_position();
//             let (center_x, center_y) = ((max_x + min_x) / 2., (max_y + min_y) / 2.);
//             let min_dist = 150. * new_zoom;
//             if let Some((_, _, width, height)) = offset.read().offset() {
//                 let max_zoom =
//                     ((max_x - min_x + min_dist) / width).max((max_y - min_y + min_dist) / height);
//                 new_zoom /= max_zoom;
//                 zoom.set_current(new_zoom);

//                 //zoom to fit nodes around center of container
//                 NODES_STORE.write().zoom_shift(
//                     zoom.zoom_factor(),
//                     (width / 2., height / 2.),
//                     (center_x, center_y),
//                 );

//                 EDGES.write().zoom_shift(
//                     zoom.zoom_factor(),
//                     (width / 2., height / 2.),
//                     (center_x, center_y),
//                 );
//             }
//         } else {
//             zoom.set_current(1.);
//         }
//     }
// }
