//! Example actix-web server that streams `optoscene` frames to a three.js
//! viewer.
//!
//! It shows the embedding pattern only: the library produces frame bytes, the
//! application ships them. The same pattern works with axum, Tauri or Dioxus
//! IPC — only the transport changes.
//!
//! Run with `cargo run -p actix-viewer`, then open <http://127.0.0.1:8080>.
//! Every two seconds the lens is nudged along the optical axis and the focused
//! ray bundle is recomputed, simulating a positioning run; the diff is streamed
//! to every connected viewer.
#![forbid(unsafe_code)]

use std::sync::Mutex;
use std::time::Duration;

use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use actix_ws::Message;
use nalgebra::{Isometry3, Point3};
use optoscene::{
    diff, fixtures, full_scene, Envelope, Layer, Material, RayStyle, RayTrace, Scene, SceneNode,
    SceneOptions,
};
use tokio::sync::broadcast;

/// The TCP address the server binds to.
const BIND: (&str, u16) = ("127.0.0.1", 8080);

/// The number of frames buffered per subscriber before it lags.
const FRAME_BUFFER: usize = 64;

/// Shared server state: the authoritative scene and the frame broadcast channel.
struct AppState {
    /// The current scene, updated by the demo loop and read on new connections.
    scene: Mutex<Scene>,
    /// Broadcasts encoded update frames to all connected viewers.
    frames: broadcast::Sender<Vec<u8>>,
}

/// Builds the demo scene: a glass lens at `lens_z` plus a focused ray bundle
/// entering at the lens, drawn as lines with a crude round envelope.
///
/// The origin is pinned so moving the lens does not shift the effective origin
/// (which would force a full resend instead of a transform update).
fn build_scene(lens_z: f64) -> Scene {
    let mut scene = Scene::new(SceneOptions {
        name: "viewer".to_string(),
        origin: Some(Point3::new(0.0, 0.0, 0.05)),
        ..SceneOptions::default()
    });

    let glass = scene.add_material(Material::Glass {
        color: [0.9, 0.95, 1.0],
        ior: 1.5,
        thickness: 0.01,
    });
    let lens = scene
        .add_mesh(fixtures::biconvex_lens(0.05, 0.01, 0.1, 0.1, 48, glass))
        .expect("valid lens mesh");
    scene
        .add_node(SceneNode {
            uid: "lens".to_string(),
            name: "lens".to_string(),
            mesh: Some(lens),
            transform: Isometry3::translation(0.0, 0.0, lens_z),
            layer: Layer::Optics,
            data: None,
        })
        .expect("unique lens node");

    scene
        .add_ray_trace(
            &focused_bundle(lens_z),
            &RayStyle {
                max_lines: 60,
                envelope: Envelope::default_round(),
                ..RayStyle::default()
            },
        )
        .expect("valid ray trace");

    scene
}

/// A focused bundle entering at the lens plane `lens_z` and converging 30 mm
/// behind it, so the bundle follows the lens as it moves.
fn focused_bundle(lens_z: f64) -> RayTrace {
    let mut trace = fixtures::focusing_bundle(0.005);
    trace.uid = "beam".to_string();
    trace.wavelength = Some(532e-9);
    for station in &mut trace.stations {
        for point in station.iter_mut().flatten() {
            point.z += lens_z;
        }
    }
    trace
}

/// Serves the viewer HTML.
async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(include_str!("../static/index.html"))
}

/// Serves the viewer script.
async fn viewer_js() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .body(include_str!("../static/viewer.js"))
}

/// Serves the current scene as a standalone GLB file.
async fn scene_glb(state: web::Data<AppState>) -> Result<HttpResponse, actix_web::Error> {
    let glb = {
        let scene = state
            .scene
            .lock()
            .map_err(|_| actix_web::error::ErrorInternalServerError("scene lock poisoned"))?;
        scene
            .to_glb()
            .map_err(actix_web::error::ErrorInternalServerError)?
    };
    Ok(HttpResponse::Ok()
        .content_type("model/gltf-binary")
        .body(glb))
}

/// Upgrades to a WebSocket, sends the full scene, then forwards update frames.
#[allow(clippy::future_not_send)]
async fn ws(
    req: HttpRequest,
    body: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    let (response, mut session, mut stream) = actix_ws::handle(&req, body)?;

    // Subscribe and snapshot the full scene under the same lock the demo loop
    // holds while broadcasting, so no update is lost or double-applied.
    let (full_frame, mut frames) = {
        let scene = state
            .scene
            .lock()
            .map_err(|_| actix_web::error::ErrorInternalServerError("scene lock poisoned"))?;
        let frames = state.frames.subscribe();
        let message = full_scene(&scene).map_err(actix_web::error::ErrorInternalServerError)?;
        drop(scene);
        let frame = message
            .to_frame()
            .map_err(actix_web::error::ErrorInternalServerError)?;
        (frame, frames)
    };

    actix_web::rt::spawn(async move {
        if session.binary(full_frame).await.is_err() {
            return;
        }
        loop {
            tokio::select! {
                incoming = stream.recv() => match incoming {
                    Some(Ok(Message::Ping(bytes))) => {
                        if session.pong(&bytes).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                    Some(Ok(_)) => {}
                },
                frame = frames.recv() => match frame {
                    Ok(bytes) => {
                        if session.binary(bytes).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                },
            }
        }
        let _ = session.close(None).await;
    });

    Ok(response)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let (frames, _keepalive) = broadcast::channel::<Vec<u8>>(FRAME_BUFFER);
    let state = web::Data::new(AppState {
        scene: Mutex::new(build_scene(0.0)),
        frames: frames.clone(),
    });

    // Demo loop: nudge the lens along z every two seconds and broadcast the diff.
    let loop_state = state.clone();
    actix_web::rt::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(2));
        let mut lens_z = 0.0_f64;
        loop {
            ticker.tick().await;
            lens_z = if lens_z >= 0.04 { 0.0 } else { lens_z + 0.005 };
            let next = build_scene(lens_z);
            let messages = match loop_state.scene.lock() {
                Ok(mut scene) => {
                    let messages = diff(&scene, &next).unwrap_or_default();
                    *scene = next;
                    messages
                }
                Err(_) => continue,
            };
            for message in &messages {
                if let Ok(frame) = message.to_frame() {
                    let _ = loop_state.frames.send(frame);
                }
            }
        }
    });

    println!("optoscene actix-viewer on http://{}:{}", BIND.0, BIND.1);
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/", web::get().to(index))
            .route("/viewer.js", web::get().to(viewer_js))
            .route("/scene.glb", web::get().to(scene_glb))
            .route("/ws", web::get().to(ws))
    })
    .bind(BIND)?
    .run()
    .await
}
