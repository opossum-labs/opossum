//! The **3D View** menu in the top navigation bar.

use dioxus::prelude::*;

use crate::{
    SCENE_VIEW_CONTROLS, SCENE_VIEW_OPEN, SCENE_VIEW_REQUEST,
    components::{
        menu_bar::menu_bar_component::AppCommand,
        scene_view::{SceneShortcut, SceneViewRequest},
    },
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
                    label: "3D view",
                    shown: Some(open),
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
                    shortcut: SceneShortcut::Fit.key_hint(),
                    on_click: move |()| *SCENE_VIEW_REQUEST.write() = Some(SceneViewRequest::FitView),
                }
                SceneViewMenuEntry {
                    label: "Reset camera",
                    disabled: !open,
                    shortcut: SceneShortcut::Reset.key_hint(),
                    on_click: move |()| {
                        *SCENE_VIEW_REQUEST.write() = Some(SceneViewRequest::ResetCamera);
                    },
                }
                li {
                    hr { class: "dropdown-divider" }
                }
                SceneViewMenuEntry {
                    label: "Table",
                    shown: Some(controls.table),
                    disabled: !open,
                    shortcut: SceneShortcut::Table.key_hint(),
                    on_click: move |()| {
                        let mut controls = SCENE_VIEW_CONTROLS.write();
                        controls.table = !controls.table;
                    },
                }
                SceneViewMenuEntry {
                    label: "World axes",
                    shown: Some(controls.axes),
                    disabled: !open,
                    shortcut: SceneShortcut::Axes.key_hint(),
                    on_click: move |()| {
                        let mut controls = SCENE_VIEW_CONTROLS.write();
                        controls.axes = !controls.axes;
                    },
                }
                SceneViewMenuEntry {
                    label: "Optical axis",
                    shown: Some(controls.beam_axis),
                    disabled: !open,
                    shortcut: SceneShortcut::Beam.key_hint(),
                    on_click: move |()| {
                        let mut controls = SCENE_VIEW_CONTROLS.write();
                        controls.beam_axis = !controls.beam_axis;
                    },
                }
                SceneViewMenuEntry {
                    label: "Rays",
                    shown: Some(controls.rays),
                    disabled: !open,
                    shortcut: SceneShortcut::Rays.key_hint(),
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
                            min: "0.01",
                            max: "1",
                            step: "0.01",
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

/// One entry of the 3D View menu: an action or a toggle, the key that also triggers it from the
/// focused viewport, and the greyed-out state the whole menu takes on while the view is closed.
///
/// Its own component rather than `MenuListItemShortCut`, which takes its label and its hint from a
/// `ShortCutAction` in the global registry - the 3D view's keys are local to it and deliberately kept
/// out of that registry.
///
/// A toggle names its state in its own wording rather than with a check mark: `shown` puts **Hide**
/// in front of the label while what it switches is on and **Show** while it is off, so the entry
/// always says what a click would do.
///
/// # Arguments
///
/// * `label`    - The entry's text for an action, or the noun a toggle's **Show**/**Hide** goes in
///   front of.
/// * `on_click` - Called on a click, unless the entry is disabled.
/// * `shown`    - `Some(true)`/`Some(false)` marks a toggle and says whether what it switches is on;
///   `None` (the default) is a plain action whose label is shown as is.
/// * `disabled` - Whether the entry is greyed out and inert.
/// * `shortcut` - The key that triggers the same action from the focused viewport, shown muted on the
///   right. Left off for the entries with no key of their own.
#[component]
fn SceneViewMenuEntry(
    label: String,
    on_click: EventHandler<()>,
    #[props(default)] shown: Option<bool>,
    #[props(default)] disabled: bool,
    #[props(default)] shortcut: Option<&'static str>,
) -> Element {
    let text = match shown {
        Some(true) => format!("Hide {label}"),
        Some(false) => format!("Show {label}"),
        None => label,
    };
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
                {text}
                if let Some(key) = shortcut {
                    span { class: "text-muted ms-4", {key} }
                }
            }
        }
    }
}
