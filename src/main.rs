//! crrDB MCP
//!
//! Features: DDL & DML executions
//!
//! NOTE: stdin/stdout is for MCP communications. Use stderr for logging.

use std::path::PathBuf;

use crrdb_mcp::{Db, Server};
use rmcp::{ServiceExt, transport::stdio};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::INFO)
        .init();

    let path = db_path();
    info!(db_path = %path.display(), "starting crrdb-mcp");

    let server = Server::new(Db::open(&path)?).serve(stdio()).await?;
    server.waiting().await?;

    info!("crrdb-mcp shutting down...");
    Ok(())
}

fn db_path() -> PathBuf {
    if let Ok(path) = std::env::var("CRRDB_PATH") {
        return PathBuf::from(path);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".local/share/crrdb/crrdb.sqlite")
}
