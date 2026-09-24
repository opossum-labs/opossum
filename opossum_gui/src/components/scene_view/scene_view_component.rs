//! The 3D view of the current model.

use dioxus::prelude::*;
use dioxus_glb_viewer::{
    Environment, GlbObject, GlbViewer, PickEvent, ViewerEvent, ViewerOptions, use_glb_viewer_handle,
};

use crate::{
    HTTP_API_CLIENT, OPOSSUM_UI_LOGS, api, api::eval_action_run, components::scene_view::objects_of,
};

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
    let viewer_handle = use_glb_viewer_handle();
    let mut reloads = use_signal(|| 0_usize);

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

    // Reading `reloads` inside is what makes the button work: the future re-runs whenever it
    // changes, and once on mount so the tab is never shown empty.
    use_future(move || async move {
        reloads();
        let manifest = api::get_scene_manifest(None).await;
        eval_action_run(
            manifest,
            Some(move |manifest| {
                objects.set(objects_of(&manifest, HTTP_API_CLIENT().base_url()));
            }),
        );
    });

    rsx! {
        div { class: "scene-view",
            div { class: "scene-view-toolbar",
                button {
                    class: "btn btn-sm btn-outline-light",
                    onclick: move |_| *reloads.write() += 1,
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
                span { class: "scene-view-count", "{objects.read().len()} components" }
            }
            GlbViewer {
                style: "flex:1; min-height:0;",
                objects,
                handle: viewer_handle,
                options,
                on_pick: move |picked: PickEvent| {
                    // Selection lives with the caller, not with the viewer: it only reports the
                    // click. Clicking empty space clears it.
                    for object in objects.write().iter_mut() {
                        object.selected = Some(&object.id) == picked.id.as_ref();
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
