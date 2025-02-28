use opossum_backend::server::start_server;
use std::error::Error;

#[actix_web::main]
async fn main() -> core::result::Result<(), impl Error> {
    start_server().await
}
