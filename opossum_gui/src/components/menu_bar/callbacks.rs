// use dioxus::{desktop::use_window, prelude::*};

// pub fn use_on_double_click(
//     mut maximize_symbol: Signal<&'static str>,
// ) -> impl FnMut(Event<MouseData>) {
//     let window = use_window();
//     move |e: Event<MouseData>| {
//         e.prevent_default();

//         if window.is_maximized() {
//             maximize_symbol.set("🗖");
//             window.set_maximized(false);
//         } else {
//             maximize_symbol.set("🗗");
//             window.set_maximized(true);
//         }
//     }
// }

// pub fn use_on_mouse_up(mut is_dragging: Signal<bool>) -> impl FnMut(Event<MouseData>) {
//     move |e: Event<MouseData>| {
//         e.prevent_default();
//         is_dragging.set(false);
//     }
// }
// pub fn use_on_mouse_down(mut is_dragging: Signal<bool>) -> impl FnMut(Event<MouseData>) {
//     move |e: Event<MouseData>| {
//         e.prevent_default();
//         is_dragging.set(true);
//     }
// }
// pub fn use_on_mouse_move(is_dragging: Signal<bool>) -> impl FnMut(Event<MouseData>) {
//     let window = use_window();
//     move |e: Event<MouseData>| {
//         e.prevent_default();
//         if is_dragging() {
//             let _ = window.drag_window();
//         }
//     }
// }
