//! The **3D View** menu in the top navigation bar.

use dioxus::prelude::*;
use dioxus_free_icons::{Icon, icons::fa_solid_icons::FaCheck};

use crate::{
    SCENE_VIEW_CONTROLS, SCENE_VIEW_OPEN, SCENE_VIEW_REQUEST,
    components::{menu_bar::menu_bar_component::AppCommand, scene_view::SceneViewRequest},
};

/// Everything the 3D view can be told, in one menu: whether it is open at all, what it shows, and
/// the three things that move its camera.
///
/// The entries below the first one need a view to act on, so they are disabled while it is closed -
/// disabled rather than hidden, so the menu always has the same shape and says what the view offers
/// before it is opened.
///
/// # Arguments
///
/// * `on_menu_action` - Where the open/close entry sends [`AppCommand::ToggleSceneView`]. That one
///   goes the long way round, through the app's command handling, because opening the view is what
///   needs the backend - everything else here only restyles what is already on screen.
/// * `is_connected`   - Whether the backend is reachable. Only the open/close entry cares.
#[component]
pub fn SceneViewMenu(on_menu_action: EventHandler<AppCommand>, is_connected: bool) -> Element {
    let open = SCENE_VIEW_OPEN();
    let controls = SCENE_VIEW_CONTROLS();
    // The two value controls are inputs rather than entries, so they carry the greying out
    // themselves; `pe-none` is left off deliberately - a disabled input already ignores the pointer.
    let control_class = if open {
        "scene-view-menu-control"
    } else {
        "scene-view-menu-control text-muted"
    };

    rsx! {
        li { class: "nav-item dropdown",
            a {
                "data-mdb-dropdown-init": "",
                "data-mdb-toggle": "dropdown",
                // Several of these are usually changed in one visit - show the rays, fade them, then
                // frame them - so a click inside the menu must not close it. Only one outside does.
                "data-mdb-auto-close": "outside",
                class: "nav-link dropdown-toggle link-secondary hidden-arrow",
                id: "navbarDropdownSceneViewMenuLink",
                role: "button",
                "3D View"
            }
            ul { class: "dropdown-menu",
                SceneViewMenuEntry {
                    label: "Show 3D view",
                    checked: open,
                    disabled: !is_connected,
                    on_click: move |()| on_menu_action.call(AppCommand::ToggleSceneView),
                }
                li {
                    hr { class: "dropdown-divider" }
                }
                SceneViewMenuEntry {
                    label: "Refresh",
                    disabled: !open,
                    on_click: move |()| *SCENE_VIEW_REQUEST.write() = Some(SceneViewRequest::Refresh),
                }
                SceneViewMenuEntry {
                    label: "Fit view",
                    disabled: !open,
                    on_click: move |()| *SCENE_VIEW_REQUEST.write() = Some(SceneViewRequest::FitView),
                }
                SceneViewMenuEntry {
                    label: "Reset camera",
                    disabled: !open,
                    on_click: move |()| {
                        *SCENE_VIEW_REQUEST.write() = Some(SceneViewRequest::ResetCamera);
                    },
                }
                li {
                    hr { class: "dropdown-divider" }
                }
                SceneViewMenuEntry {
                    label: "Table",
                    checked: controls.table,
                    disabled: !open,
                    on_click: move |()| {
                        let mut controls = SCENE_VIEW_CONTROLS.write();
                        controls.table = !controls.table;
                    },
                }
                SceneViewMenuEntry {
                    label: "Axes",
                    checked: controls.axes,
                    disabled: !open,
                    on_click: move |()| {
                        let mut controls = SCENE_VIEW_CONTROLS.write();
                        controls.axes = !controls.axes;
                    },
                }
                SceneViewMenuEntry {
                    label: "Beam axis",
                    checked: controls.beam_axis,
                    disabled: !open,
                    on_click: move |()| {
                        let mut controls = SCENE_VIEW_CONTROLS.write();
                        controls.beam_axis = !controls.beam_axis;
                    },
                }
                SceneViewMenuEntry {
                    label: "Rays",
                    checked: controls.rays,
                    disabled: !open,
                    on_click: move |()| {
                        let mut controls = SCENE_VIEW_CONTROLS.write();
                        controls.rays = !controls.rays;
                    },
                }
                li {
                    hr { class: "dropdown-divider" }
                }
                li { class: "dropdown-item-text",
                    label { class: control_class,
                        "max. rays"
                        input {
                            r#type: "number",
                            class: "form-control form-control-sm",
                            style: "width: 5.5rem;",
                            min: "1",
                            step: "50",
                            disabled: !open,
                            value: "{controls.max_rays}",
                            // On commit (Enter or leaving the field), not per keystroke: every new
                            // number is a new trace.
                            onchange: move |e| {
                                if let Ok(n) = e.value().parse::<usize>() {
                                    SCENE_VIEW_CONTROLS.write().max_rays = n.max(1);
                                }
                            },
                        }
                    }
                }
                li { class: "dropdown-item-text",
                    label { class: control_class,
                        "ray opacity"
                        input {
                            r#type: "range",
                            class: "form-range",
                            style: "width: 6rem;",
                            min: "0",
                            max: "1",
                            step: "0.05",
                            disabled: !open,
                            value: "{controls.opacity}",
                            // Only restyles what is on screen: nothing is fetched or traced again.
                            oninput: move |e| {
                                if let Ok(v) = e.value().parse::<f32>() {
                                    SCENE_VIEW_CONTROLS.write().opacity = v;
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

/// One entry of the 3D View menu: a label, a check mark while whatever it switches is on, and the
/// greyed-out state the whole menu takes on while the view is closed.
///
/// Its own component rather than `MenuListItemShortCut`, which takes its label and its hint from a
/// `ShortCutAction` - none of these entries has a shortcut, and none of the existing entries shows
/// state.
///
/// # Arguments
///
/// * `label`    - The entry's text.
/// * `on_click` - Called on a click, unless the entry is disabled.
/// * `checked`  - Whether to draw the check mark. Left off for the entries that are actions.
/// * `disabled` - Whether the entry is greyed out and inert.
#[component]
fn SceneViewMenuEntry(
    label: String,
    on_click: EventHandler<()>,
    #[props(default)] checked: bool,
    #[props(default)] disabled: bool,
) -> Element {
    rsx! {
        li {
            a {
                class: if disabled { "dropdown-item d-flex justify-content-between align-items-center disabled pe-none text-muted" } else { "dropdown-item d-flex justify-content-between align-items-center" },
                role: "button",
                "aria-disabled": disabled,
                onclick: move |_| {
                    if !disabled {
                        on_click.call(());
                    }
                },
                {label}
                if checked {
                    Icon { width: 12, height: 12, icon: FaCheck }
                }
            }
        }
    }
}
