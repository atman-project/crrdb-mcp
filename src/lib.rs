mod db;
mod error;
#[cfg(feature = "ffi")]
pub mod ffi;
mod params;
mod server;

pub use db::Db;
pub use error::Error;
pub use server::Server;

// Must come after `ffi` so its `#[uniffi::export]` metadata is in scope.
#[cfg(feature = "ffi")]
uniffi::setup_scaffolding!();
