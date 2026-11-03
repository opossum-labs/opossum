//! The 3D view of the current model.

use std::collections::HashSet;

use dioxus::prelude::*;
use dioxus_glb_viewer::{
    GlbObject, GlbViewer, OverlayBlending, PickEvent, ViewerEvent, ViewerOptions,
    use_glb_viewer_handle,
};
use opossum_core::types::api_types::{
    AnalyzerItemDto, SceneAnalyzer, SceneManifest, SceneSpectrum, SkippedSceneNode,
};

use crate::{
    CONTEXT_MENU, HTTP_API_CLIENT, OPOSSUM_UI_LOGS, SCENE_REVISION, SCENE_VIEW_ANALYZER,
    SCENE_VIEW_CONTROLS, SCENE_VIEW_REQUEST, api,
    api::eval_action_run,
    components::{
        context_menu::cx_menu::CxMenu,
        scene_view::{
            AXIS_OBJECT_ID, RAYS_OBJECT_ID, SceneShortcut, SceneViewRequest, action_for_pick,
            apply_node_selection, is_ray_object, objects_of, ray_object, set_environment,
            table_ground,
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
const SETTLE: std::time::Duration = std::time::Duration::from_millis(100);

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
    // The components the last fetch had to leave out, each with why and the graph it sits in.
    let mut skipped = use_signal(Vec::<SkippedSceneNode>::new);
    // Why nothing of the model can be drawn, as the last fetch said; `None` while it can be.
    let mut notice = use_signal(|| None::<String>);
    // The analyzer the last fetch was placed after. The axis and the rays follow the same one, and
    // the status line names it.
    let mut drawn_after = use_signal(|| None::<SceneAnalyzer>);
    // The false colors that analyzer's light is drawn in, for the legend; `None` for one wavelength.
    let mut spectrum = use_signal(|| None::<SceneSpectrum>);
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

    // How often the model has been fetched; only the first fetch is not debounced.
    let mut fetches = use_signal(|| 0_usize);
    // The rays as the menu has them drawn - how many, and whether every wavelength at each start
    // point - or `None` while they are hidden. The manifest digests the rays only for these.
    let drawn_rays = use_memo(move || {
        let controls = SCENE_VIEW_CONTROLS();
        controls
            .rays
            .then_some((controls.max_rays, controls.full_spectrum))
    });

    let mut options = use_signal(|| {
        let mut options = ViewerOptions {
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
        };
        // Without an environment every lens renders black: glass refracts its surroundings, and an
        // empty scene has none. See `dioxus_glb_viewer`'s `Environment`.
        set_environment(&mut options, SCENE_VIEW_CONTROLS.peek().hdr_environment);
        options
    });

    // Reading `SCENE_REVISION` inside is what keeps the view live: the resource re-runs on every
    // model change, on every other choice of analyzer or of drawn rays, and once on mount so the tab
    // is never shown empty. It only exists while the tab is open, so a closed 3D view costs the
    // backend nothing. The rays asked for are kept with the manifest, as its digest is theirs.
    let mut manifest = use_resource(move || async move {
        SCENE_REVISION();
        let analyzer = SCENE_VIEW_ANALYZER();
        let rays = drawn_rays();
        // A burst of edits - dragging a value through a range, or a paste that touches several
        // nodes - would otherwise re-mesh the whole model once per step. Restarting the resource
        // drops the sleeping future, so only the last edit of a burst survives to fetch anything.
        // The first load is not delayed: there is nothing on screen yet to keep steady.
        if *fetches.peek() > 0 {
            debounce().await;
        }
        api::get_scene_manifest(analyzer, rays)
            .await
            .map(|manifest| (manifest, rays))
    });

    use_effect(move || {
        if let Some(fetched) = &*manifest.read() {
            // Cloned out of the resource before the callback runs, so the borrow is not held across
            // the write to `objects`.
            eval_action_run(
                fetched.clone(),
                Some(move |(fetched, rays): (SceneManifest, _)| {
                    let fetch = *fetches.peek() + 1;
                    fetches.set(fetch);
                    drawn_after.set(fetched.analyzer.clone());
                    spectrum.set(fetched.spectrum.clone());
                    notice.set(fetched.notice.clone());
                    skipped.set(fetched.skipped.clone());
                    for s in &fetched.skipped {
                        OPOSSUM_UI_LOGS.write().add_log(&format!(
                            "3D view: '{}' could not be drawn: {}",
                            s.name, s.reason
                        ));
                    }
                    // Handing over a whole new list is the point: the viewer compares it against
                    // what it already draws and moves, reloads or removes only what differs. The
                    // light is named by its digests, so light that did not change is not fetched
                    // again; a model that cannot be placed has none.
                    let base_url = HTTP_API_CLIENT().base_url().to_owned();
                    let controls = *SCENE_VIEW_CONTROLS.peek();
                    let analyzer = fetched.analyzer.as_ref().map(|analyzer| analyzer.uid);
                    let mut list = objects_of(&fetched, &base_url);
                    if let Some(axis) = &fetched.axis {
                        list.push(ray_object(
                            AXIS_OBJECT_ID,
                            api::scene_axis_url(&base_url, axis, analyzer),
                            controls.beam_axis,
                            // Always solid, whatever the rays are set to: the axis is the single
                            // line the whole setup is built on, and it is one line per source
                            // rather than a bundle that needs thinning out to be read.
                            1.0,
                        ));
                    }
                    if let (Some(digest), Some((max_rays, full_spectrum))) = (&fetched.rays, rays) {
                        list.push(ray_object(
                            RAYS_OBJECT_ID,
                            api::scene_rays_url(
                                &base_url,
                                digest,
                                max_rays,
                                full_spectrum,
                                analyzer,
                            ),
                            controls.rays,
                            controls.opacity,
                        ));
                    }
                    // Box whatever the graph has selected right now, so the selection box survives a
                    // model refetch while a node stays selected. Read by `peek`, so this effect stays
                    // tied to the manifest alone and a mere selection change never triggers a refetch.
                    let selected = selected_ids.peek();
                    apply_node_selection(&mut list, &selected);
                    objects.set(list);
                }),
            );
        }
    });

    // The one place the menu's settings reach the view. Both signals are written only where they
    // really differ: a write of `options` resends the whole scene's options to the renderer, and a
    // write of `objects` re-renders the viewer to compute a diff that would come out empty.
    use_effect(move || {
        let controls = SCENE_VIEW_CONTROLS();
        let mut wanted = options.peek().clone();
        wanted.ground = controls.table.then(table_ground);
        wanted.orientation_gizmo = controls.axes;
        wanted.overlay_blending = if controls.additive_rays {
            OverlayBlending::Additive
        } else {
            OverlayBlending::Ordered
        };
        set_environment(&mut wanted, controls.hdr_environment);
        if *options.peek() != wanted {
            options.set(wanted);
        }
        // Built from the list on screen rather than from the manifest: what is drawn is whatever the
        // last fetch produced, and the menu only shows, hides and fades the light over it. A new
        // number of rays or way of drawing their spectrum fetches the model again instead (see
        // `drawn_rays`). The opacity is the rays' alone, the axis staying solid.
        let mut list = objects.peek().clone();
        for object in &mut list {
            if object.id == AXIS_OBJECT_ID {
                object.visible = controls.beam_axis;
            } else if object.id == RAYS_OBJECT_ID {
                object.visible = controls.rays;
                object.opacity = controls.opacity;
            }
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

    // The legend of the false colors, while the rays they color are shown: the shortest and longest
    // wavelength, to a tenth of a nanometer, around a bar of the colors in between.
    let legend = spectrum()
        .filter(|_| SCENE_VIEW_CONTROLS.read().rays)
        .map(|spectrum| {
            let stops = spectrum
                .colors
                .iter()
                .map(|[r, g, b]| format!("rgb({:.0} {:.0} {:.0})", r * 255., g * 255., b * 255.))
                .collect::<Vec<_>>()
                .join(", ");
            (
                (spectrum.shortest_nm * 10.).round() / 10.,
                format!("background: linear-gradient(to right, {stops});"),
                (spectrum.longest_nm * 10.).round() / 10.,
            )
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
            // What the view has to say about itself: the analyzer it follows, why nothing could be
            // drawn, what was left out, and how much is drawn. The analyzer is chosen here, where its
            // effect shows, and what was left out is listed here to jump to in the graph; everything
            // else the view can be told sits in the 3D View menu.
            div { class: "scene-view-status",
                if let Some(analyzer) = drawn_after() {
                    span {
                        class: "scene-view-analyzer",
                        title: "The analyzer the 3D view is drawn after. Click to follow another.",
                        onclick: move |event: MouseEvent| {
                            let (x, y) = (event.page_coordinates().x, event.page_coordinates().y);
                            spawn(async move {
                                eval_action_run(
                                    api::get_analyzers().await,
                                    Some(move |analyzers: Vec<AnalyzerItemDto>| {
                                        let mut menu = CxMenu::new(x, y, vec![]);
                                        for AnalyzerItemDto { id, info } in analyzers {
                                            let mark = if id == analyzer.uid {
                                                "\u{25cf}"
                                            } else {
                                                "\u{2003}"
                                            };
                                            menu.add_view_entry(
                                                format!("{mark} {}", info.display_name()),
                                                Callback::new(move |()| {
                                                    *SCENE_VIEW_ANALYZER.write() = Some(id);
                                                }),
                                            );
                                        }
                                        *CONTEXT_MENU.write() = Some(menu);
                                    }),
                                );
                            });
                        },
                        "{analyzer.name} \u{25be}"
                    }
                }
                if let Some((shortest, bar, longest)) = legend {
                    span {
                        class: "scene-view-spectrum",
                        title: "The light is drawn in false colors, from blue at the shortest wavelength of the sources to red at the longest.",
                        "{shortest} nm"
                        span { class: "scene-view-spectrum-bar", style: "{bar}" }
                        "{longest} nm"
                    }
                }
                if let Some(text) = notice() {
                    span { class: "scene-view-notice", title: "{text}", "{text}" }
                }
                if !skipped.read().is_empty() {
                    span {
                        class: "scene-view-skipped",
                        title: "Click to list them and go to one in the graph.",
                        onclick: move |event: MouseEvent| {
                            let mut menu = CxMenu::new(
                                event.page_coordinates().x,
                                event.page_coordinates().y,
                                vec![],
                            );
                            // Cloned out first, so no borrow of the signal outlives this line.
                            let components = skipped.peek().clone();
                            for component in components {
                                menu.add_view_entry(
                                    format!("{} ({})", component.name, component.reason),
                                    Callback::new(move |()| {
                                        workspace_processor
                                            .send(GraphsWorkspaceAction::RevealNode {
                                                node_id: component.uid,
                                                graph_id: component.group,
                                                bring_to_front: true,
                                            });
                                    }),
                                );
                            }
                            *CONTEXT_MENU.write() = Some(menu);
                        },
                        "{skipped.read().len()} component(s) could not be drawn \u{2013} click to list"
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
                    if let Some(Ok((fetched, _))) = &*manifest.read()
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
                                .add_log(
                                    &format!("the 3D view could not draw component {id}: {message}"),
                                );
                        }
                        ViewerEvent::Error { message } => {
                            OPOSSUM_UI_LOGS
                                .write()
                                .add_log(&format!("the 3D view reported: {message}"));
                        }
                        _ => {}
                    }
                },
            }
        }
    }
}
