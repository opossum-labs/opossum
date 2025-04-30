use dioxus::prelude::*;

#[component]
pub fn ZoomShiftContainer() -> Element {
    let mut shift=use_signal(||(0,0));
    let mut is_dragging=use_signal(||false);
    let mut current_mouse_pos=use_signal(||(0,0));
    let mut zoom=use_signal(||1.0);
    rsx!{
        div {
            id: "editor",
            style:  "background-color: #e5e5f7; width: 600px; height: 400px; overflow: hidden;",
            onwheel: move |event| {
                let delta = event.delta().strip_units().y;
                if delta > 0.0 {
                     zoom *=  1.1
                } else {
                     zoom /= 1.1
                };
            },
            onmousedown: move |event| {
                current_mouse_pos.set((event.client_coordinates().x as i32, event.client_coordinates().y as i32));
                is_dragging.set(true);
            },
            onmouseup: move |_| {
                is_dragging.set(false);
            },
            onmousemove: move |event| {
                if is_dragging() {
                    let rel_shift_x = event.client_coordinates().x as i32 - current_mouse_pos().0;
                    let rel_shift_y = event.client_coordinates().y as i32 - current_mouse_pos().1;
                    current_mouse_pos.set((event.client_coordinates().x as i32, event.client_coordinates().y as i32));
                    shift.set((shift().0 + rel_shift_x, shift().1 + rel_shift_y));
                }
            },
            div {
                class: "zoom-shift-container",
                style: format!("transform: translate({}px, {}px) scale({zoom});", shift().0, shift().1),
                div {
                    id: "node1",
                    style: "background-color: red; position: absolute; width: 50px; height: 30px; left: 100px; top: 100px;",
                }
                div {
                    id: "node2",
                    style: "background-color: blue; position: absolute; width: 50px; height: 30px; left: 150px; top: 150px;",
                }
            }
        }
    }
}