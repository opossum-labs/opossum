//! The 3D view of the current model.

use std::collections::HashSet;

use dioxus::prelude::*;
use dioxus_glb_viewer::{
    Environment, GlbObject, GlbViewer, PickEvent, ViewerEvent, ViewerOptions, use_glb_viewer_handle,
};

use crate::{
    HTTP_API_CLIENT, OPOSSUM_UI_LOGS, SCENE_REVISION, SCENE_VIEW_CONTROLS, SCENE_VIEW_REQUEST, api,
    api::eval_action_run,
    components::{
        scene_view::{
            AXIS_OBJECT_ID, RAYS_OBJECT_ID, SceneShortcut, SceneViewRequest, action_for_pick,
            apply_node_selection, is_ray_object, objects_of, ray_object, table_ground,
        },
        scenery_editor::{
            GraphStateStoreExt, GraphsWorkspaceAction, GraphsWorkspaceState,
            GraphsWorkspaceStateStoreExt,
        },
    },
};

/// How long the view waits for the edits to stop before asking for the model again.
///
/// Long enough that holding a spinner or dragging a value through a range costs one fetch rather
/// than dozens, short enough that a single deliberate edit still feels immediate.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(300);

/// Wait out [`SETTLE`], on whichever platform this is.
async fn debounce() {
    #[cfg(feature = "desktop")]
    tokio::time::sleep(SETTLE).await;
    #[cfg(not(feature = "desktop"))]
    gloo_timers::future::sleep(SETTLE).await;
}

/// Show the model's components in three dimensions.
///
/// The component holds the object list itself and hands it to [`GlbViewer`], which works out the
/// smallest set of updates from one list to the next. That is why the list is rebuilt wholesale on
/// every refresh rather than patched here: rebuilding it is cheap, and an object whose id, source
/// and transform are unchanged produces no update at all.
///
/// The geometry is never carried through this component. Each object names a URL, and the webview
/// fetches it directly from the backend.
#[component]
pub fn SceneView() -> Element {
    let mut objects = use_signal(Vec::<GlbObject>::new);
    let mut skipped_count = use_signal(|| 0_usize);
    let viewer_handle = use_glb_viewer_handle();
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    // The graph editor provides its workspace store to this whole subtree, so the 3D view can read
    // which nodes are selected in the graph and box the same ones - the reverse of what `on_pick`
    // does. See `graph_editor_component`'s `use_context_provider`.
    let workspace = use_context::<ReadStore<GraphsWorkspaceState>>();
    // The nodes selected in the active graph tab, deduped: this recomputes on any change to the
    // active tab's store but only notifies when the selected set itself differs, so dragging a node
    // does not re-run the selection effect below. Mirrors `graph_editor_component`'s
    // `selected_nodes_memo`, and tracks only the active tab just as the properties sidebar does.
    let selected_ids = use_memo(move || {
        let active = *workspace.active_tab().read();
        workspace
            .tabs()
            .get(active)
            .map_or_else(HashSet::new, |g| g.graph_store().read().selected_node_ids())
    });
    // The 3D view's own keyboard shortcuts act only while its viewport holds focus, so the container
    // below is made focusable and its handle kept here to focus it on a click - see its `onkeydown`.
    let mut viewport = use_signal(|| None::<std::rc::Rc<MountedData>>);

    // How often the model has been fetched. The axis URL carries it, so every fetch of the
    // components fetches the axis again too; see `api::scene_axis_url`.
    let mut fetches = use_signal(|| 0_usize);
    // The rays object as the current fetch and the menu's ray count and opacity describe it.
    let rays_object = move || {
        let controls = *SCENE_VIEW_CONTROLS.peek();
        ray_object(
            RAYS_OBJECT_ID,
            api::scene_rays_url(
                HTTP_API_CLIENT().base_url(),
                *fetches.peek(),
                controls.max_rays,
            ),
            true,
            controls.opacity,
        )
    };

    let mut options = use_signal(|| ViewerOptions {
        // Without this, every lens renders black: glass refracts its surroundings, and an empty
        // scene has none. See `dioxus_glb_viewer`'s `Environment`.
        environment: Environment::Room,
        background: "#435d7a".into(),
        // The model is metres across and sits near the origin, while the default grid is twenty
        // units wide - it would swamp the setup rather than give it a floor.
        grid: false,
        fit_on_first_load: true,
        // Show a corner gizmo so the user always knows which way is up in the 3D view.
        orientation_gizmo: SCENE_VIEW_CONTROLS.peek().axes,
        // The optical table as a floor reaching to the horizon; see `table_ground`.
        ground: SCENE_VIEW_CONTROLS.peek().table.then(table_ground),
        // Drawn light - the optical axis - is a line; at one pixel it disappears against the table.
        line_width: 3.0,
        ..ViewerOptions::default()
    });

    // Reading `SCENE_REVISION` inside is what keeps the view live: the resource re-runs on every
    // model change, and once on mount so the tab is never shown empty. It only exists while the tab
    // is open, so a closed 3D view costs the backend nothing at all.
    let mut manifest = use_resource(move || async move {
        SCENE_REVISION();
        // A burst of edits - dragging a value through a range, or a paste that touches several
        // nodes - would otherwise re-mesh the whole model once per step. Restarting the resource
        // drops the sleeping future, so only the last edit of a burst survives to fetch anything.
        // The first load is not delayed: there is nothing on screen yet to keep steady.
        if !objects.peek().is_empty() {
            debounce().await;
        }
        api::get_scene_manifest(None).await
    });

    use_effect(move || {
        if let Some(fetched) = &*manifest.read() {
            // Cloned out of the resource before the callback runs, so the borrow is not held across
            // the write to `objects`.
            eval_action_run(
                fetched.clone(),
                Some(move |fetched| {
                    // Handing over a whole new list is the point: the viewer compares it against
                    // what it already draws and moves, reloads or removes only what differs.
                    let base_url = HTTP_API_CLIENT().base_url().to_owned();
                    let fetch = *fetches.peek() + 1;
                    fetches.set(fetch);
                    let controls = *SCENE_VIEW_CONTROLS.peek();
                    let mut list = objects_of(&fetched, &base_url);
                    list.push(ray_object(
                        AXIS_OBJECT_ID,
                        api::scene_axis_url(&base_url, fetch),
                        controls.beam_axis,
                        // Always solid, whatever the rays are set to: the axis is the single line the
                        // whole setup is built on, and it is one line per source rather than a bundle
                        // that needs thinning out to be read.
                        1.0,
                    ));
                    if controls.rays {
                        list.push(rays_object());
                    }
                    // Box whatever the graph has selected right now, so the selection box survives a
                    // model refetch while a node stays selected. Read by `peek`, so this effect stays
                    // tied to the manifest alone and a mere selection change never triggers a refetch.
                    let selected = selected_ids.peek();
                    apply_node_selection(&mut list, &selected);
                    objects.set(list);
                    skipped_count.set(fetched.skipped.len());
                    for s in &fetched.skipped {
                        OPOSSUM_UI_LOGS.write().add_log(&format!(
                            "3D view: '{}' could not be drawn: {}",
                            s.name, s.reason
                        ));
                    }
                }),
            );
        }
    });

    // The one place the menu's settings reach the view. Both signals are written only where they
    // really differ: a write of `options` resends the whole scene's options to the renderer, and a
    // write of `objects` re-renders the viewer to compute a diff that would come out empty.
    use_effect(move || {
        let controls = SCENE_VIEW_CONTROLS();
        let ground = controls.table.then(table_ground);
        let differs = {
            let options = options.peek();
            options.ground != ground || options.orientation_gizmo != controls.axes
        };
        if differs {
            let mut options = options.write();
            options.ground = ground;
            options.orientation_gizmo = controls.axes;
        }
        // Built from the list on screen rather than from the manifest: which components are drawn is
        // whatever the last fetch produced, and only the light over them is the menu's business. The
        // rays object is dropped and rebuilt so that a changed ray count comes out as a new URL, and
        // with it the opacity - which is the rays' alone, the axis staying solid.
        let mut list = objects.peek().clone();
        list.retain(|object| object.id != RAYS_OBJECT_ID);
        for object in &mut list {
            if object.id == AXIS_OBJECT_ID {
                object.visible = controls.beam_axis;
            }
        }
        if controls.rays {
            list.push(rays_object());
        }
        if list != *objects.peek() {
            objects.set(list);
        }
    });

    // Reflect the graph's selection into the 3D box - the reverse of `on_pick`. The selection is
    // read reactively and the object list by peek, so this re-runs when the selection changes but
    // not when it writes the list back; the equality guard means an unchanged selection costs the
    // viewer no update. A model refetch bakes the selection in itself (see the rebuild effect
    // above), so this effect only carries a selection change made without one.
    use_effect(move || {
        let selected = selected_ids();
        let mut list = objects.peek().clone();
        apply_node_selection(&mut list, &selected);
        if list != *objects.peek() {
            objects.set(list);
        }
    });

    // Carrying out what the menu asked for once. The slot is cleared before the request is acted on,
    // so a request is never carried out twice, and the borrow taken to read it is long gone by then.
    use_effect(move || {
        let Some(request) = *SCENE_VIEW_REQUEST.read() else {
            return;
        };
        *SCENE_VIEW_REQUEST.write() = None;
        match request {
            SceneViewRequest::Refresh => manifest.restart(),
            SceneViewRequest::FitView => viewer_handle.fit_view(),
            SceneViewRequest::ResetCamera => viewer_handle.reset_camera(),
        }
    });

    rsx! {
        div {
            class: "scene-view",
            // The view's keyboard shortcuts - Fit, Reset, and the four visibility toggles - act only
            // while this container is focused, so it is made focusable and refocused on every click
            // into it: a click to orbit is enough to be "in" the view. They are kept off the global
            // shortcut listener on purpose - those bare letters must stay free for whatever panel
            // holds focus elsewhere.
            tabindex: 0,
            onmounted: move |evt| viewport.set(Some(evt.data())),
            onmousedown: move |_| {
                if let Some(node) = viewport() {
                    spawn(async move {
                        let _ = node.set_focus(true).await;
                    });
                }
            },
            onkeydown: move |evt| {
                let modifiers = evt.modifiers();
                // Bare keys only: a combo with Ctrl/Cmd/Alt/Shift is either a global shortcut or none
                // of ours, and an auto-repeat while a key is held would flip a toggle on and off.
                if modifiers.ctrl() || modifiers.meta() || modifiers.alt() || modifiers.shift()
                    || evt.is_auto_repeating()
                {
                    return;
                }
                if let Key::Character(character) = evt.key()
                    && let Some(shortcut) = SceneShortcut::from_key(&character)
                {
                    // Suppress the browser default and the bubble only for a key we act on, so an
                    // unbound key still reaches whatever else might want it.
                    evt.prevent_default();
                    evt.stop_propagation();
                    match shortcut {
                        SceneShortcut::Fit => {
                            *SCENE_VIEW_REQUEST.write() = Some(SceneViewRequest::FitView);
                        }
                        SceneShortcut::Reset => {
                            *SCENE_VIEW_REQUEST.write() = Some(SceneViewRequest::ResetCamera);
                        }
                        SceneShortcut::Table => {
                            let mut controls = SCENE_VIEW_CONTROLS.write();
                            controls.table = !controls.table;
                        }
                        SceneShortcut::Axes => {
                            let mut controls = SCENE_VIEW_CONTROLS.write();
                            controls.axes = !controls.axes;
                        }
                        SceneShortcut::Beam => {
                            let mut controls = SCENE_VIEW_CONTROLS.write();
                            controls.beam_axis = !controls.beam_axis;
                        }
                        SceneShortcut::Rays => {
                            let mut controls = SCENE_VIEW_CONTROLS.write();
                            controls.rays = !controls.rays;
                        }
                    }
                }
            },
            // What the view has to say about itself, now that everything it can be told sits in the
            // 3D View menu: what was left out, and how much is drawn.
            div { class: "scene-view-status",
                if *skipped_count.read() > 0 {
                    span {
                        class: "scene-view-skipped",
                        "{skipped_count} component(s) could not be drawn \u{2013} see logs"
                    }
                }
                span { class: "scene-view-count",
                    "{objects.read().iter().filter(|o| !is_ray_object(&o.id)).count()} components"
                }
            }
            GlbViewer {
                // `.dxglb-root` brings `height: 100%`, which in this column would mean the full
                // panel height *plus* the status strip above it. A flex basis of zero takes the
                // height from the flexbox instead, which is the only one that knows what is left
                // over.
                style: "flex: 1 1 0; min-height: 0; height: auto;",
                objects,
                handle: viewer_handle,
                options,
                on_pick: move |picked: PickEvent| {
                    // Selection lives with the caller, not with the viewer: it only reports the
                    // click. Clicking empty space clears it. Drawn rays are never model nodes and
                    // must never receive a selection box.
                    for object in objects.write().iter_mut() {
                        object.selected =
                            !is_ray_object(&object.id) && Some(&object.id) == picked.id.as_ref();
                    }
                    // Mirror the 2D canvas: a hit opens the component's properties without leaving
                    // the 3D view, and a click on empty space clears the selection so the
                    // properties sidebar deselects too - not just the 3D box. See `action_for_pick`.
                    // Looked up in the manifest last fetched rather than awaiting a fresh one, so
                    // the click responds immediately.
                    if let Some(Ok(fetched)) = &*manifest.read()
                        && let Some(action) = action_for_pick(&picked, fetched)
                    {
                        workspace_processor.send(action);
                    }
                },
                on_event: move |event: ViewerEvent| {
                    // A model that fails to load would otherwise be an object that silently never
                    // appears, with nothing anywhere saying why.
                    match event {
                        ViewerEvent::LoadError { id, message } => {
                            OPOSSUM_UI_LOGS
                                .write()
                                .add_log(&format!("the 3D view could not draw component {id}: {message}"));
                        }
                        ViewerEvent::Error { message } => {
                            OPOSSUM_UI_LOGS.write().add_log(&format!("the 3D view reported: {message}"));
                        }
                        _ => {}
                    }
                },
            }
        }
    }
}
