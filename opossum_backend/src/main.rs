mod analyzers;
mod app_state;
mod document;
mod error;
mod general;
mod helper_functions;
mod nodes;
mod operations;
mod pages;
mod pump_scenarios;
mod routes;
mod server;
mod sse_logger;
mod undo;

use std::error::Error;

#[actix_web::main]
async fn main() -> core::result::Result<(), impl Error> {
    server::start().await
}
