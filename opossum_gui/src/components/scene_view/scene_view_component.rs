//! The 3D view of the current model.

use dioxus::prelude::*;
use dioxus_glb_viewer::{
    Environment, GlbObject, GlbViewer, PickEvent, ViewerEvent, ViewerOptions, use_glb_viewer_handle,
};

use crate::{
    HTTP_API_CLIENT, OPOSSUM_UI_LOGS, SCENE_REVISION, api,
    api::eval_action_run,
    components::{
        scene_view::{objects_of, reveal_action_for_pick},
        scenery_editor::GraphsWorkspaceAction,
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

    let options = use_signal(|| ViewerOptions {
        // Without this, every lens renders black: glass refracts its surroundings, and an empty
        // scene has none. See `dioxus_glb_viewer`'s `Environment`.
        environment: Environment::Room,
        background: "#1a1a2e".into(),
        // The model is metres across and sits near the origin, while the default grid is twenty
        // units wide - it would swamp the setup rather than give it a floor.
        grid: false,
        fit_on_first_load: true,
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
                    objects.set(objects_of(&fetched, HTTP_API_CLIENT().base_url()));
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

    rsx! {
        div { class: "scene-view",
            div { class: "scene-view-toolbar",
                button {
                    class: "btn btn-sm btn-outline-light",
                    onclick: move |_| manifest.restart(),
                    "Refresh"
                }
                button {
                    class: "btn btn-sm btn-outline-light",
                    onclick: move |_| viewer_handle.fit_view(),
                    "Fit view"
                }
                button {
                    class: "btn btn-sm btn-outline-light",
                    onclick: move |_| viewer_handle.reset_camera(),
                    "Reset camera"
                }
                if *skipped_count.read() > 0 {
                    span {
                        class: "scene-view-skipped",
                        "{skipped_count} component(s) could not be drawn \u{2013} see logs"
                    }
                }
                span { class: "scene-view-count", "{objects.read().len()} components" }
            }
            GlbViewer {
                // `.dxglb-root` brings `height: 100%`, which in this column would mean the full
                // panel height *plus* the toolbar above it. A flex basis of zero takes the height
                // from the flexbox instead, which is the only one that knows what is left over.
                style: "flex: 1 1 0; min-height: 0; height: auto;",
                objects,
                handle: viewer_handle,
                options,
                on_pick: move |picked: PickEvent| {
                    // Selection lives with the caller, not with the viewer: it only reports the
                    // click. Clicking empty space clears it.
                    for object in objects.write().iter_mut() {
                        object.selected = Some(&object.id) == picked.id.as_ref();
                    }
                    // A hit also opens the component's properties, without leaving the 3D view -
                    // see `reveal_action_for_pick`. Looked up in the manifest last fetched rather
                    // than awaiting a fresh one, so the click responds immediately.
                    if let Some(Ok(fetched)) = &*manifest.read()
                        && let Some(action) = reveal_action_for_pick(&picked, fetched)
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
