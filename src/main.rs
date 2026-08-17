//! crrDB MCP
//!
//! Features: DDL & DML executions
//!
//! NOTE: stdin/stdout is for MCP communications. Use stderr for logging.

use tracing::info;

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("starting crrdb-mcp");
}
