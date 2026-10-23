// In opossum_backend/src/payload_logger.rs

use actix_http::h1;
use actix_web::{
    Error, HttpMessage, HttpResponse,
    body::{BoxBody, MessageBody, to_bytes},
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::{Method, StatusCode, header},
    web::BytesMut,
};
use futures_util::StreamExt;
use futures_util::future::{LocalBoxFuture, Ready, ready};
use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::Mutex;

/// Standard banner display width in terminal characters
const BANNER_WIDTH: usize = 76;

/// ANSI terminal color definitions
mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const DIM: &str = "\x1b[2m";

    pub const BOLD_CYAN: &str = "\x1b[1;36m";
    pub const BOLD_MAGENTA: &str = "\x1b[1;35m";
    pub const BOLD_GREEN: &str = "\x1b[1;32m";
    pub const BOLD_YELLOW: &str = "\x1b[1;33m";
    pub const BOLD_RED: &str = "\x1b[1;31m";
}

/// Helper function to center a title between '=' characters with consistent width and ANSI coloring.
fn create_banner(title: &str, color: &str) -> String {
    let padded_title = if title.is_empty() {
        String::new()
    } else {
        format!(" {title} ")
    };
    let centered = format!("{padded_title:=^BANNER_WIDTH$}");
    format!("{color}{centered}{}", colors::RESET)
}

/// Returns an ANSI color string matching the HTTP method semantics.
const fn method_color(method: &Method) -> &'static str {
    match *method {
        Method::GET => colors::BOLD_CYAN,
        Method::POST => colors::BOLD_GREEN,
        Method::PUT | Method::PATCH => colors::BOLD_YELLOW,
        Method::DELETE => colors::BOLD_RED,
        _ => colors::BOLD_MAGENTA,
    }
}

/// Returns an ANSI color string matching the HTTP status code category.
fn status_color(status: StatusCode) -> &'static str {
    if status.is_success() {
        colors::BOLD_GREEN
    } else if status.is_redirection() {
        colors::BOLD_YELLOW
    } else if status.is_client_error() || status.is_server_error() {
        colors::BOLD_RED
    } else {
        colors::RESET
    }
}

/// Formats raw payload bytes into a human-readable representation.
pub fn format_payload(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "<empty>".to_string();
    }

    let text = match std::str::from_utf8(bytes) {
        Ok(valid_str) => valid_str.trim(),
        Err(_) => return format!("<binary data: {} bytes>", bytes.len()),
    };

    if text.is_empty() {
        return "<empty>".to_string();
    }

    // 1. Attempt JSON pretty-printing
    if ((text.starts_with('{') && text.ends_with('}'))
        || (text.starts_with('[') && text.ends_with(']')))
        && let Ok(json_val) = serde_json::from_str::<serde_json::Value>(text)
        && let Ok(pretty) = serde_json::to_string_pretty(&json_val)
    {
        return pretty;
    }

    // 2. Attempt RON pretty-printing
    if let Ok(ron_val) = ron::from_str::<ron::Value>(text) {
        let pretty_cfg = ron::ser::PrettyConfig::default();
        if let Ok(pretty) = ron::ser::to_string_pretty(&ron_val, pretty_cfg) {
            return pretty;
        }
    }

    // 3. Fallback: Return raw string
    text.to_string()
}

/// Actix-Web middleware transform enforcing strictly sequential execution and colored payload logging.
#[derive(Clone)]
pub struct PayloadLogger {
    lock: Arc<Mutex<()>>,
    counter: Arc<AtomicU64>,
}

impl PayloadLogger {
    pub fn new() -> Self {
        Self {
            lock: Arc::new(Mutex::new(())),
            counter: Arc::new(AtomicU64::new(1)),
        }
    }
}

impl Default for PayloadLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, B> Transform<S, ServiceRequest> for PayloadLogger
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type InitError = ();
    type Transform = PayloadLoggerMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(PayloadLoggerMiddleware {
            service: Rc::new(service),
            lock: Arc::clone(&self.lock),
            counter: Arc::clone(&self.counter),
        }))
    }
}

pub struct PayloadLoggerMiddleware<S> {
    service: Rc<S>,
    lock: Arc<Mutex<()>>,
    counter: Arc<AtomicU64>,
}

impl<S, B> Service<ServiceRequest> for PayloadLoggerMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        let lock = Arc::clone(&self.lock);
        let counter = Arc::clone(&self.counter);

        Box::pin(async move {
            // Acquire sequential lock across all incoming requests
            let _guard = lock.lock().await;
            let req_id = counter.fetch_add(1, Ordering::Relaxed);

            let method = req.method().clone();
            let path = req.path().to_string();

            // 1. Buffer the full request payload into memory
            let mut payload_bytes = BytesMut::new();
            let mut stream = req.take_payload();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                payload_bytes.extend_from_slice(&chunk);
            }

            let req_body = payload_bytes.freeze();

            // 2. Print formatted incoming request header
            let m_color = method_color(&method);
            let req_title = format!("DEBUG: INCOMING REQUEST #{req_id}");
            println!("{}", create_banner(&req_title, colors::BOLD_CYAN));
            println!("--> {m_color}{method}{reset} {path}", reset = colors::RESET);

            if req_body.is_empty() {
                println!(
                    "--> {dim}Payload: <empty>{reset}",
                    dim = colors::DIM,
                    reset = colors::RESET
                );
            } else {
                println!(
                    "--> {dim}Payload:{reset}\n{payload}",
                    dim = colors::DIM,
                    reset = colors::RESET,
                    payload = format_payload(&req_body)
                );
            }

            // 3. Reconstruct the request payload stream for downstream handlers
            let (_, mut new_payload) = h1::Payload::create(true);
            new_payload.unread_data(req_body);
            req.set_payload(new_payload.into());

            // 4. Forward the request to the inner service pipeline
            let res = service.call(req).await?;

            // 5. Deconstruct the response to inspect the response body
            let (req, res) = res.into_parts();
            let status = res.status();
            let (res_head, body) = res.into_parts();

            // 6. Protect Server-Sent Events (SSE) from buffering
            let is_sse = res_head
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|h| h.to_str().ok())
                .is_some_and(|ct| ct.contains("text/event-stream"));

            let resp_title = format!("DEBUG: OUTGOING RESPONSE #{req_id}");

            if is_sse {
                println!("{}", create_banner(&resp_title, colors::BOLD_MAGENTA));
                println!(
                    "<-- {s_color}{status}{reset} for {m_color}{method}{reset} {path} [SSE Stream Active]",
                    s_color = status_color(status),
                    reset = colors::RESET
                );
                println!("{}\n", create_banner("", colors::BOLD_MAGENTA));

                let mut new_res = HttpResponse::new(status);
                *new_res.headers_mut() = res_head.headers().clone();
                let new_res = new_res.set_body(BoxBody::new(body));
                return Ok(ServiceResponse::new(req, new_res));
            }

            // 7. Buffer standard response body
            let res_bytes = match to_bytes(body).await {
                Ok(bytes) => bytes,
                Err(err) => {
                    let boxed_err: Box<dyn std::error::Error> = err.into();
                    eprintln!("Failed to read response body for debugging: {boxed_err}");
                    actix_web::web::Bytes::new()
                }
            };

            // 8. Print formatted outgoing response
            println!("{}", create_banner(&resp_title, colors::BOLD_MAGENTA));
            println!(
                "<-- {s_color}{status}{reset} for {m_color}{method}{reset} {path}",
                s_color = status_color(status),
                reset = colors::RESET
            );
            if res_bytes.is_empty() {
                println!(
                    "<-- {dim}Payload: <empty>{reset}",
                    dim = colors::DIM,
                    reset = colors::RESET
                );
            } else {
                println!(
                    "<-- {dim}Payload:{reset}\n{payload}",
                    dim = colors::DIM,
                    reset = colors::RESET,
                    payload = format_payload(&res_bytes)
                );
            }
            // Closing line with identical length plus an empty line for readability
            println!("{}\n", create_banner("", colors::BOLD_MAGENTA));

            // 9. Reconstruct the response with a boxed body
            let mut new_res = HttpResponse::new(status);
            *new_res.headers_mut() = res_head.headers().clone();
            let new_res = new_res.set_body(BoxBody::new(res_bytes));

            Ok(ServiceResponse::new(req, new_res))
        })
    }
}
