// In opossum_backend/src/main.rs

mod analyzers;
mod app_state;
mod document;
mod error;
mod general;
mod helper_functions;
mod nodes;
mod operations;
mod pages;
mod payload_logger;
mod pump_scenarios;
mod routes;
mod server;
mod sse_logger;
mod undo;

use std::error::Error;

#[actix_web::main]
async fn main() -> core::result::Result<(), impl Error> {
    let args: Vec<String> = std::env::args().collect();
    let debug_payload = args
        .iter()
        .any(|arg| arg == "--debug-payload" || arg == "-d");

    if debug_payload {
        println!("[OPOSSUM] Detailed payload debugging enabled (--debug-payload).");
    }

    let config = server::ServerConfig { debug_payload };
    server::start_with_config(&config).await
}
