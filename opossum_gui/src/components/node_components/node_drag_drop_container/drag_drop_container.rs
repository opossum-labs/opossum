use crate::components::node_components::{
    edges::edges_component::{EdgeCreation, EdgeCreationComponent, EdgesComponent},
    node_drag_drop_container::callbacks::{
        use_on_double_click, use_on_key_down, use_on_mounted, use_on_mouse_move, use_on_mouse_up,
        use_on_resize, use_on_wheel,
    },
    nodes::Nodes,
    DraggedNode, NodeOffset,
};
use dioxus::{html::geometry::WheelDelta, prelude::*};
use std::rc::Rc;

fn use_init_signals() {
    use_context_provider(|| Signal::new(None::<Rc<MountedData>>));
    use_context_provider(|| Signal::new(DraggedNode::new(None, None)));
    use_context_provider(|| Signal::new(NodeOffset::new(None)));
    use_context_provider(|| Signal::new(None::<EdgeCreation>));
}

#[component]
pub fn NodeDragDropContainer() -> Element {
    use_init_signals();

    rsx! {
        div {
            onmounted: use_on_mounted(),
            onmousemove: use_on_mouse_move(),
            onmouseup: use_on_mouse_up(),
            onwheel: use_on_wheel(),
            onresize: use_on_resize(),
            ondoubleclick: use_on_double_click(),
            onkeydown: use_on_key_down(),
            tabindex: "0",
            class: "drop-container",
            id: "drag_drop_container",
            Nodes {}

            svg { width: "100%", height: "100%", class: "edge-creation",
                {
                    rsx! {
                        EdgesComponent {}
                        EdgeCreationComponent {}
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Zoom {
    current: f64,
    previous: f64,
}

impl Zoom {
    pub fn new(current: f64, previous: f64) -> Self {
        Self { current, previous }
    }
    pub fn current(&self) -> f64 {
        self.current
    }
    pub fn set_current(&mut self, current: f64) {
        self.previous = self.current;
        self.current = current;
    }
    pub fn previous(&self) -> f64 {
        self.previous
    }
    pub fn zoom_factor(&self) -> f64 {
        self.current / self.previous
    }

    pub fn set_zoom_from_scroll_event(&mut self, event: &WheelEvent) {
        let zoom_factor = 1.1;
        let mut new_zoom = self.current();

        let delta_sign = match event.delta() {
            WheelDelta::Pixels(px) => px.y.signum(),
            WheelDelta::Lines(li) => li.y.signum(),
            WheelDelta::Pages(pp) => pp.y.signum(),
        };

        if delta_sign.is_sign_negative() {
            new_zoom *= zoom_factor;
        } else {
            new_zoom /= zoom_factor;
        }

        new_zoom = new_zoom.clamp(0.2, 5.0);

        self.set_current(new_zoom);
    }
}

pub trait ZoomShift {
    fn zoom_shift(&mut self, zoom_factor: f64, shift: (f64, f64), zoom_center: (f64, f64));
}
