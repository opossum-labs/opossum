// src/platform/mod.rs

#[cfg(feature = "desktop")]
mod desktop;
#[cfg(feature = "desktop")]
pub use desktop::*;

#[cfg(not(feature = "desktop"))]
mod web;
#[cfg(not(feature = "desktop"))]
pub use web::*;
