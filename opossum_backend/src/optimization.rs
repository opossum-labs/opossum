use crate::{
    app_state::{AppState, OptimizationStatus},
    error::BackEndErrorResponse,
    undo::{Command, OptimizationUndo},
};
use actix_web::{HttpResponse, Responder, get, http::header, post, web};
use opossum_core::opm_document::{OpmDocument, OptimizationConfig, optimization::ParameterAddress};
use opossum_optimization::{
    OpticOptimizer, OptimizationProgressEvent, ParameterApplier, SchemaExtractor,
};
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::time::timeout;
use utoipa_actix_web::service_config::ServiceConfig;

/// Grace period before an active optimization is aborted when no client listens to `/events`.
const DISCONNECT_GRACE_PERIOD: Duration = Duration::from_secs(5);

/// Interval for sending SSE keep-alive comments to promptly detect dropped connections.
const SSE_KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(2);

/// Get schema from the current model (`variables`, `observables`, `edge_thickness_nodes`)
#[utoipa::path(responses((status = OK, description = "get optimization operands")), tag="optimization")]
#[get("/schema")]
pub async fn get_schema(data: web::Data<AppState>) -> impl Responder {
    let doc = data.document.lock();
    let schema = SchemaExtractor::extract(&doc);
    drop(doc);
    HttpResponse::Ok().json(schema)
}

/// Set optimization configuration and start optimization
#[utoipa::path(
    tag = "optimization",
    request_body(
        content = String,
        description = "OptimizationConfig in RON format", 
        content_type = "application/ron"
    ),
    responses(
        (status = ACCEPTED, description = "Optimization job started")
    )
)]
#[post("/start")]
pub async fn start_optimization(
    data: web::Data<AppState>,
    body: String,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let opt_config: OptimizationConfig = ron::from_str(&body)
        .or_else(|ron_err| serde_json::from_str(&body).map_err(|_| ron_err))
        .map_err(|e| {
            BackEndErrorResponse::new(
                400,
                "Parse Error",
                &format!("Invalid optimization payload: {e}"),
            )
        })?;

    {
        let mut status = data.optimization.status.write();
        if *status == OptimizationStatus::Running {
            return Err(BackEndErrorResponse::new(
                409,
                "Conflict",
                "An optimization job is already running",
            ));
        }
        *status = OptimizationStatus::Running;
    }
    data.optimization.cancel_flag.store(false, Ordering::SeqCst);

    let (working_doc, original_values) = {
        let mut doc = data.document.lock();
        doc.set_optimization_config(Some(opt_config.clone()));
        let snapshot = snapshot_blueprint_and_values(&doc, &opt_config);
        drop(doc);
        snapshot
    };

    spawn_optimization_worker(data, working_doc, opt_config, original_values);

    Ok(HttpResponse::Accepted().json("Optimization started"))
}

/// Abort a running optimization job
#[utoipa::path(responses((status = OK, description = "abort a running optimization process")), tag="optimization")]
#[post("/abort")]
pub async fn abort_optimization(data: web::Data<AppState>) -> impl Responder {
    data.optimization.cancel_flag.store(true, Ordering::SeqCst);
    HttpResponse::Ok().json("Abort requested")
}

/// Receive live updates of the optimization progress
#[utoipa::path(responses((status = OK, description = "get live events during the optimization process")), tag="optimization")]
#[get("/events")]
pub async fn optimization_events(data: web::Data<AppState>) -> impl Responder {
    let rx = data.optimization.event_sender.subscribe();
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            // Send a periodic SSE comment if no optimization event arrives so Actix-Web
            // detects closed TCP sockets promptly and drops the broadcast receiver.
            match timeout(SSE_KEEP_ALIVE_INTERVAL, rx.recv()).await {
                Ok(Ok(event)) => match serde_json::to_string(&event) {
                    Ok(json) => {
                        let bytes = web::Bytes::from(format!("data: {json}\n\n"));
                        return Some((Ok::<_, actix_web::Error>(bytes), rx));
                    }
                    Err(e) => {
                        log::error!("Failed to serialize OptimizationProgressEvent: {e}");
                    }
                },
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => return None,
                Err(_) => {
                    let bytes = web::Bytes::from_static(b": keep-alive\n\n");
                    return Some((Ok::<_, actix_web::Error>(bytes), rx));
                }
            }
        }
    });

    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "text/event-stream"))
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .streaming(stream)
}

fn snapshot_blueprint_and_values(
    doc: &OpmDocument,
    config: &OptimizationConfig,
) -> (OpmDocument, HashMap<ParameterAddress, f64>) {
    let mut original_values = HashMap::new();
    for var in &config.free_variables {
        if let Ok(val) = opossum_optimization::MetricReader::read_parameter_value(doc, &var.address)
        {
            original_values.insert(var.address.clone(), val);
        }
    }
    for c in &config.couplings {
        if let Ok(val) = opossum_optimization::MetricReader::read_parameter_value(doc, &c.target) {
            original_values.insert(c.target.clone(), val);
        }
    }
    (doc.clone(), original_values)
}

fn apply_results_and_record_undo(
    app_state: &AppState,
    optimized_parameters: &HashMap<ParameterAddress, f64>,
    original_values: &HashMap<ParameterAddress, f64>,
) {
    let mut diff = HashMap::new();
    {
        let mut doc = app_state.document.lock();
        for (addr, new_val) in optimized_parameters {
            let old_val = original_values.get(addr).copied().unwrap_or(*new_val);
            diff.insert(addr.clone(), (old_val, *new_val));
            let _ = ParameterApplier::set_value(&mut doc, addr, *new_val);
        }
        let _ = ParameterApplier::finalize_update(&mut doc);
        let _ = doc.analyze_headless();
    }

    let inverse_diff: HashMap<_, _> = diff
        .into_iter()
        .map(|(addr, (old_val, new_val))| (addr, (new_val, old_val)))
        .collect();

    app_state.push_undo(Command::ApplyOptimization(OptimizationUndo {
        parameter_diff: inverse_diff,
    }));
}

/// Monitor active SSE subscribers and abort the optimization if no client listens
/// continuously for `grace_period`.
fn spawn_subscriber_watchdog(
    tx: broadcast::Sender<OptimizationProgressEvent>,
    cancel_flag: Arc<AtomicBool>,
    disconnect_aborted: Arc<AtomicBool>,
    done_flag: Arc<AtomicBool>,
    grace_period: Duration,
) {
    tokio::spawn(async move {
        let mut no_listener_since: Option<Instant> = Some(Instant::now());
        let mut interval = tokio::time::interval(Duration::from_millis(500));

        loop {
            interval.tick().await;

            if done_flag.load(Ordering::SeqCst) || cancel_flag.load(Ordering::SeqCst) {
                break;
            }

            if tx.receiver_count() == 0 {
                let start = no_listener_since.get_or_insert_with(Instant::now);
                if start.elapsed() >= grace_period {
                    log::warn!(
                        "No active optimization event listeners for {grace_period:?}. Aborting optimization."
                    );
                    disconnect_aborted.store(true, Ordering::SeqCst);
                    cancel_flag.store(true, Ordering::SeqCst);
                    break;
                }
            } else {
                no_listener_since = None;
            }
        }
    });
}

fn spawn_optimization_worker(
    app_state: web::Data<AppState>,
    mut working_doc: OpmDocument,
    opt_config: OptimizationConfig,
    original_values: HashMap<ParameterAddress, f64>,
) {
    let cancel_flag = app_state.optimization.cancel_flag.clone();
    let tx = app_state.optimization.event_sender.clone();
    let disconnect_aborted = Arc::new(AtomicBool::new(false));
    let done_flag = Arc::new(AtomicBool::new(false));

    spawn_subscriber_watchdog(
        tx.clone(),
        cancel_flag.clone(),
        disconnect_aborted.clone(),
        done_flag.clone(),
        DISCONNECT_GRACE_PERIOD,
    );

    tokio::task::spawn_blocking(move || {
        // Ensure `done_flag` is always set when the blocking worker exits so the watchdog stops.
        struct DoneGuard(Arc<AtomicBool>);
        impl Drop for DoneGuard {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let _done_guard = DoneGuard(done_flag);

        let optimizer = match OpticOptimizer::from_config(&mut working_doc, opt_config) {
            Ok(opt) => opt,
            Err(e) => {
                let _ = tx.send(OptimizationProgressEvent::Failed {
                    error: e.to_string(),
                });
                *app_state.optimization.status.write() = OptimizationStatus::Failed;
                return;
            }
        };

        let tx_clone = tx.clone();
        let opt_result = optimizer.optimize_with_observer(cancel_flag.clone(), move |event| {
            let _ = tx_clone.send(event);
            true
        });

        let was_disconnected = disconnect_aborted.load(Ordering::SeqCst);

        match opt_result {
            Ok(summary) => {
                let is_aborted = cancel_flag.load(Ordering::SeqCst);
                if is_aborted {
                    // Only apply intermediate best parameters if aborted manually by the user.
                    // If aborted due to client disconnect, leave the document untouched.
                    if !was_disconnected {
                        apply_results_and_record_undo(
                            &app_state,
                            &summary.optimized_parameters,
                            &original_values,
                        );
                    }
                    *app_state.optimization.status.write() = OptimizationStatus::Aborted;
                    let _ = tx.send(OptimizationProgressEvent::Aborted {
                        last_cost: summary.final_cost,
                        iterations: summary.iterations,
                        best_parameters: summary.optimized_parameters.into_iter().collect(),
                    });
                } else {
                    apply_results_and_record_undo(
                        &app_state,
                        &summary.optimized_parameters,
                        &original_values,
                    );
                    *app_state.optimization.status.write() = OptimizationStatus::Finished;
                    let _ = tx.send(OptimizationProgressEvent::Finished {
                        final_cost: summary.final_cost,
                        iterations: summary.iterations,
                        converged: summary.converged,
                        optimized_parameters: summary.optimized_parameters.into_iter().collect(),
                    });
                }
            }
            Err(err) => {
                let err_str = err.to_string();
                if err_str.contains("cancelled by user") || was_disconnected {
                    *app_state.optimization.status.write() = OptimizationStatus::Aborted;
                    let _ = tx.send(OptimizationProgressEvent::Aborted {
                        last_cost: f64::NAN,
                        iterations: 0,
                        best_parameters: Vec::new(),
                    });
                } else {
                    *app_state.optimization.status.write() = OptimizationStatus::Failed;
                    let _ = tx.send(OptimizationProgressEvent::Failed { error: err_str });
                }
            }
        }
    });
}

/// Set optimization configuration on current document
#[utoipa::path(
    tag = "optimization",
    request_body(
        content = String,
        description = "OptimizationConfig in RON format",
        content_type = "application/ron"
    ),
    responses(
        (status = OK, description = "Optimization configuration saved to document")
    )
)]
#[post("/config")]
pub async fn save_config(
    data: web::Data<AppState>,
    body: String,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let opt_config: OptimizationConfig = ron::from_str(&body)
        .or_else(|ron_err| serde_json::from_str(&body).map_err(|_| ron_err))
        .map_err(|e| {
            BackEndErrorResponse::new(
                400,
                "Parse Error",
                &format!("Invalid optimization payload: {e}"),
            )
        })?;

    data.document
        .lock()
        .set_optimization_config(Some(opt_config));
    Ok(HttpResponse::Ok().json("Optimization config saved"))
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_schema);
    cfg.service(save_config);
    cfg.service(start_optimization);
    cfg.service(abort_optimization);
    cfg.service(optimization_events);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::document::undo_document;
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::{
        distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
        joule,
        light::lightdata::ray_data_source::{PointSrc, RayDataSource},
        millimeter, nanometer,
        nodes::{Lens, SourcePort, SpotDiagram},
        opm_document::optimization::{
            FreeVariable, MetricOperand, ObservableAddress, OperandMode, OptimizationAlgorithm,
        },
        prelude::{AnalyzerType, RayTraceConfig, RefrIndexSchott},
    };

    /// Build minimal optical test setup in AppState
    fn setup_test_app() -> (Data<AppState>, uuid::Uuid, uuid::Uuid, uuid::Uuid) {
        let app_state = Data::new(AppState::default());
        let (lens_id, spot_id, analyzer_id) = {
            let mut doc = app_state.document.lock();
            let scenery = doc.scenery_mut();

            let src_id = scenery.add_node(SourcePort::new("source")).unwrap();

            let refr = RefrIndexSchott::new(
                3.267,
                -2.05e-2,
                3.51e-2,
                7.70e-3,
                -9.08e-4,
                7.52e-5,
                nanometer!(300.0)..nanometer!(2000.0),
            )
            .unwrap();

            let lens = Lens::new(
                "test_lens",
                millimeter!(100.0),
                millimeter!(-100.0),
                millimeter!(10.0),
                &refr,
            )
            .unwrap();
            let lens_id = scenery.add_node(lens).unwrap();

            let spot = SpotDiagram::new("spot").unwrap();
            let spot_id = scenery.add_node(spot).unwrap();

            scenery
                .connect_nodes(src_id, "output_1", lens_id, "input_1", millimeter!(70.0))
                .unwrap();
            scenery
                .connect_nodes(lens_id, "output_1", spot_id, "input_1", millimeter!(50.0))
                .unwrap();

            let mut rt_config = RayTraceConfig::default();
            rt_config.map_source(
                src_id,
                RayDataSource::PointSrc(
                    PointSrc::new(
                        Hexapolar::new(millimeter!(10.0), 3).unwrap().into(),
                        UniformDist::new(joule!(1.0)).unwrap().into(),
                        LaserLines::new(vec![(nanometer!(1000.0), 1.0)])
                            .unwrap()
                            .into(),
                        millimeter!(70.0),
                    )
                    .unwrap(),
                )
                .into(),
            );
            let analyzer_id = doc.add_analyzer(AnalyzerType::RayTrace(rt_config));

            (lens_id, spot_id, analyzer_id)
        };

        (app_state, lens_id, spot_id, analyzer_id)
    }

    #[actix_web::test]
    async fn test_optimization_schema_endpoint() {
        let (app_state, _, _, _) = setup_test_app();
        let app = test::init_service(App::new().app_data(app_state).service(get_schema)).await;

        let req = test::TestRequest::get().uri("/schema").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = test::read_body(resp).await;
        let schema: opossum_optimization::ModelOptimizationSchema =
            serde_json::from_slice(&body).unwrap();

        assert!(
            !schema.controllable_variables.is_empty(),
            "There must be controllable variables"
        );
        assert!(
            !schema.observable_metrics.is_empty(),
            "There must be detector metrics"
        );
    }

    #[actix_web::test]
    async fn test_optimization_lifecycle_and_undo() {
        let (app_state, lens_id, spot_id, analyzer_id) = setup_test_app();
        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(start_optimization)
                .service(abort_optimization)
                .service(undo_document),
        )
        .await;

        let dist_addr = ParameterAddress::ConnectionDistance {
            src_id: lens_id,
            src_port: "output_1".into(),
        };

        let initial_dist = {
            let doc = app_state.document.lock();
            opossum_optimization::MetricReader::read_parameter_value(&doc, &dist_addr).unwrap()
        };

        let config = OptimizationConfig::new()
            .with_variable(
                FreeVariable::new(dist_addr.clone())
                    .with_length_bounds(millimeter!(30.0), millimeter!(90.0))
                    .unwrap(),
            )
            .with_operand(MetricOperand::for_spot_radius(
                ObservableAddress {
                    analyzer_id,
                    node_id: spot_id,
                    metric_name: "rms beam radius".into(),
                },
                opossum_core::micrometer!(0.0),
                OperandMode::Equal,
                1.0,
            ))
            .with_algorithm(OptimizationAlgorithm::nelder_mead().with_max_iterations(15));

        let mut rx = app_state.optimization.event_sender.subscribe();

        let req = test::TestRequest::post()
            .uri("/start")
            .set_payload(ron::to_string(&config).unwrap())
            .insert_header(("Content-Type", "application/ron"))
            .to_request();

        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);

        let mut got_started = false;
        let mut got_iteration = false;
        let mut got_finished = false;

        for _ in 0..50 {
            if let Ok(event) = rx.recv().await {
                match event {
                    OptimizationProgressEvent::Started { .. } => got_started = true,
                    OptimizationProgressEvent::Iteration { .. } => got_iteration = true,
                    OptimizationProgressEvent::Finished { .. } => {
                        got_finished = true;
                        break;
                    }
                    OptimizationProgressEvent::Failed { error } => {
                        panic!("Optimization failed: {error}");
                    }
                    _ => {}
                }
            }
        }

        assert!(got_started, "should receive start event");
        assert!(got_iteration, "should receive at least on iteration event");
        assert!(got_finished, "should receive finished event");

        let new_dist = {
            let doc = app_state.document.lock();
            opossum_optimization::MetricReader::read_parameter_value(&doc, &dist_addr).unwrap()
        };
        assert!(
            (new_dist - initial_dist).abs() > 1e-6,
            "distance should have been optimized"
        );

        let undo_req = test::TestRequest::post().uri("/undo").to_request();
        let undo_resp = app.call(undo_req).await.unwrap();
        assert_eq!(undo_resp.status(), StatusCode::OK);

        let restored_dist = {
            let doc = app_state.document.lock();
            opossum_optimization::MetricReader::read_parameter_value(&doc, &dist_addr).unwrap()
        };
        assert!(
            (restored_dist - initial_dist).abs() < 1e-12,
            "undo should restore the initial setup"
        );
    }

    #[actix_web::test]
    async fn test_start_optimization_with_ron_special_floats() {
        let (app_state, lens_id, spot_id, analyzer_id) = setup_test_app();
        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(start_optimization)
                .service(abort_optimization),
        )
        .await;

        let dist_addr = ParameterAddress::ConnectionDistance {
            src_id: lens_id,
            src_port: "output_1".into(),
        };

        // Configuration with Infinity and NaN values
        let config = OptimizationConfig::new()
            .with_variable(
                FreeVariable::new(dist_addr)
                    .with_length_bounds(millimeter!(30.0), millimeter!(90.0))
                    .unwrap(),
            )
            .with_operand(MetricOperand::for_spot_radius(
                ObservableAddress {
                    analyzer_id,
                    node_id: spot_id,
                    metric_name: "rms beam radius".into(),
                },
                opossum_core::micrometer!(0.0),
                OperandMode::Equal,
                f64::INFINITY,
            ))
            .with_target_cost(f64::INFINITY)
            .with_algorithm(OptimizationAlgorithm::nelder_mead().with_max_iterations(1));

        // In JSON, Infinity or NaN is serialized to `null`, which causes deserialization
        // into f64 to fail with: invalid type: null, expected f64.
        let json_payload = serde_json::to_string(&config).unwrap();
        assert!(serde_json::from_str::<OptimizationConfig>(&json_payload).is_err());

        // RON handles Infinity and NaN properly without data loss or parse failure
        let ron_payload =
            ron::to_string(&config).expect("RON serialization must support Infinity/NaN");
        assert!(ron_payload.contains("inf"));
        let deserialized_ron: OptimizationConfig =
            ron::from_str(&ron_payload).expect("RON deserialization must succeed");
        assert_eq!(deserialized_ron.target_cost, f64::INFINITY);

        let req = test::TestRequest::post()
            .uri("/start")
            .set_payload(ron_payload)
            .insert_header(("Content-Type", "application/ron"))
            .to_request();

        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
    }

    #[actix_web::test]
    async fn test_save_config_and_schema_saved_config() {
        let (app_state, _, spot_id, _) = setup_test_app();
        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(get_schema)
                .service(save_config),
        )
        .await;

        let dist_addr = ParameterAddress::ConnectionDistance {
            src_id: spot_id,
            src_port: "output_1".into(),
        };

        let config = OptimizationConfig::new()
            .with_variable(FreeVariable::new(dist_addr))
            .with_target_cost(1e-5);

        let ron_payload = ron::to_string(&config).unwrap();

        // 1. Post to save_config
        let req = test::TestRequest::post()
            .uri("/config")
            .set_payload(ron_payload)
            .insert_header(("Content-Type", "application/ron"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify document in app_state has the config
        {
            let doc = app_state.document.lock();
            assert_eq!(doc.optimization_config(), Some(&config));
        }

        // 2. Get schema and verify saved_config is populated
        let req = test::TestRequest::get().uri("/schema").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let schema: opossum_optimization::ModelOptimizationSchema =
            test::read_body_json(resp).await;
        assert_eq!(schema.saved_config, Some(config));
    }
}