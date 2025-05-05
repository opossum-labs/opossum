use crate::components::scenery_editor::{
    edges::edges_component::{EdgeCreation, EdgeCreationComponent, EdgesComponent},
    graph_editor::{
        callbacks::{use_on_double_click, use_on_key_down, use_on_mounted, use_on_resize},
        graph_editor_commands::{add_analyzer, add_node, delete_scenery},
    },
    DraggedNode, NodeOffset, Nodes,
};
use dioxus::{html::geometry::WheelDelta, prelude::*};
use opossum_backend::AnalyzerType;
use std::rc::Rc;
use uuid::Uuid;

fn use_init_signals() {
    use_context_provider(|| Signal::new(None::<Rc<MountedData>>));
    use_context_provider(|| Signal::new(DraggedNode::new(None, None)));
    use_context_provider(|| Signal::new(NodeOffset::new(None)));
    use_context_provider(|| Signal::new(None::<EdgeCreation>));
}

#[derive(Debug)]
pub enum NodeEditorCommand {
    DeleteAll,
    AddNode(String),
    AddAnalyzer(AnalyzerType),
}

#[component]
pub fn GraphEditor(
    command: ReadOnlySignal<Option<NodeEditorCommand>>,
    node_selected: Signal<Option<Uuid>>,
) -> Element {
    use_init_signals();
    let mut shift = use_signal(|| (0, 0));
    let mut is_dragging = use_signal(|| false);
    let mut current_mouse_pos = use_signal(|| (0, 0));
    let mut zoom = use_signal(|| 1.0);

    use_effect(move || {
        let command = command.read();
        if let Some(command) = &*(command) {
            match command {
                NodeEditorCommand::DeleteAll => {
                    println!("NodeEditor: Delete all nodes");
                    delete_scenery();
                }
                NodeEditorCommand::AddNode(node_type) => {
                    println!("NodeEditor: Node selected: {:?}", node_type);
                    add_node(node_type.clone(), Uuid::nil());
                }
                NodeEditorCommand::AddAnalyzer(analyzer_type) => {
                    println!("NodeEditor: Analyzer selected: {:?}", analyzer_type);
                    add_analyzer(analyzer_type.clone());
                }
            }
        }
    });
    rsx! {
        div {
            onmounted: use_on_mounted(),
            //onmousemove: use_on_mouse_move(),
            //onmouseup: use_on_mouse_up(),
            //onwheel: use_on_wheel(),
            onresize: use_on_resize(),
            ondoubleclick: use_on_double_click(),
            onkeydown: use_on_key_down(),
            onwheel: move |event| {
                let delta = event.delta().strip_units().y;
                if delta > 0.0 { zoom *= 1.1 } else { zoom /= 1.1 };
            },
            onmousedown: move |event| {
                current_mouse_pos
                    .set((
                        event.client_coordinates().x as i32,
                        event.client_coordinates().y as i32,
                    ));
                is_dragging.set(true);
            },
            onmouseup: move |_| {
                is_dragging.set(false);
            },
            onmousemove: move |event| {
                if is_dragging() {
                    let rel_shift_x = event.client_coordinates().x as i32
                        - current_mouse_pos().0;
                    let rel_shift_y = event.client_coordinates().y as i32
                        - current_mouse_pos().1;
                    current_mouse_pos
                        .set((
                            event.client_coordinates().x as i32,
                            event.client_coordinates().y as i32,
                        ));
                    shift.set((shift().0 + rel_shift_x, shift().1 + rel_shift_y));
                }
            },
            tabindex: "0",
            class: "drop-container",
            id: "drag_drop_container",
            div {
                class: "zoom-shift-container",
                style: format!("transform: translate({}px, {}px) scale({zoom});", shift().0, shift().1),

                Nodes {node_activated: node_selected}
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
}

#[derive(Clone, PartialEq)]
pub struct Zoom {
    current: f64,
    previous: f64,
}

impl Zoom {
    #[must_use]
    pub const fn new(current: f64, previous: f64) -> Self {
        Self { current, previous }
    }
    #[must_use]
    pub const fn current(&self) -> f64 {
        self.current
    }
    pub const fn set_current(&mut self, current: f64) {
        self.previous = self.current;
        self.current = current;
    }
    #[must_use]
    pub const fn previous(&self) -> f64 {
        self.previous
    }
    #[must_use]
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
