use crate::{
    api::{self},
    components::scenery_editor::{edges::edges_component::EdgeCreation, EDGES},
    HTTP_API_CLIENT, OPOSSUM_UI_LOGS,
};
use dioxus::prelude::*;
use std::rc::Rc;

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
