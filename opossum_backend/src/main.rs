use opossum_backend::server::start;
use std::error::Error;

#[actix_web::main]
async fn main() -> core::result::Result<(), impl Error> {
    start().await
}
