//! UniFFI surface — lets mobile apps embed crrdb-mcp as a library instead of
//! spawning it as a stdio MCP server. The tool contract is byte-identical to
//! the MCP surface: same names, same JSON in/out, because it reuses the exact
//! same `Server` methods.

use std::sync::Arc;

use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult};
use serde::de::DeserializeOwned;

use crate::{Db, Server, server::INSTRUCTIONS};

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum FfiError {
    #[error("failed to open database: {0}")]
    Open(String),
    #[error("unknown tool: {0}")]
    UnknownTool(String),
    #[error("invalid tool params: {0}")]
    InvalidParams(String),
}

/// The same payload an MCP client would receive: the tool's JSON result
/// (or the structured error JSON) plus the error flag.
#[derive(uniffi::Record)]
pub struct ToolOutput {
    pub json: String,
    pub is_error: bool,
}

#[derive(uniffi::Object)]
pub struct CrrdbMcp {
    server: Server,
}

#[uniffi::export]
impl CrrdbMcp {
    #[uniffi::constructor]
    pub fn new(db_path: String) -> Result<Arc<Self>, FfiError> {
        let db =
            Db::open(std::path::Path::new(&db_path)).map_err(|e| FfiError::Open(e.to_string()))?;
        Ok(Arc::new(Self {
            server: Server::new(db),
        }))
    }

    /// The MCP server instructions — use as (part of) the model's system prompt.
    pub fn instructions(&self) -> String {
        INSTRUCTIONS.to_string()
    }

    /// Tool definitions as a JSON array in Anthropic Messages API format
    /// (`{name, description, input_schema}`), taken from the same rmcp router
    /// the MCP server serves.
    pub fn tool_definitions(&self) -> String {
        let tools: Vec<serde_json::Value> = Server::tool_router()
            .list_all()
            .into_iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.input_schema,
                })
            })
            .collect();
        serde_json::Value::Array(tools).to_string()
    }

    /// Execute a tool by name with JSON-encoded input.
    pub fn execute_tool(&self, name: String, input_json: String) -> Result<ToolOutput, FfiError> {
        let result = match name.as_str() {
            "get_schema" => self.server.get_schema(),
            "sample_rows" => self.server.sample_rows(self.params(&input_json)?),
            "query" => self.server.query(self.params(&input_json)?),
            "create_table" => self.server.create_table(self.params(&input_json)?),
            "alter_table" => self.server.alter_table(self.params(&input_json)?),
            "commit_records" => self.server.commit_records(self.params(&input_json)?),
            "update_records" => self.server.update_records(self.params(&input_json)?),
            "delete_records" => self.server.delete_records(self.params(&input_json)?),
            _ => return Err(FfiError::UnknownTool(name)),
        };
        Ok(tool_output(result))
    }
}

impl CrrdbMcp {
    fn params<T: DeserializeOwned>(&self, json: &str) -> Result<Parameters<T>, FfiError> {
        let json = if json.trim().is_empty() { "{}" } else { json };
        serde_json::from_str(json)
            .map(Parameters)
            .map_err(|e| FfiError::InvalidParams(e.to_string()))
    }
}

fn tool_output(result: CallToolResult) -> ToolOutput {
    let is_error = result.is_error.unwrap_or(false);
    let json = result
        .content
        .iter()
        .filter_map(|content| content.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n");
    ToolOutput { json, is_error }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_contract() {
        let dir = std::env::temp_dir().join(format!("crrdb-ffi-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mcp = CrrdbMcp::new(dir.join("test.sqlite").to_string_lossy().into_owned()).unwrap();

        // All eight tools are exported with schemas.
        let defs: serde_json::Value = serde_json::from_str(&mcp.tool_definitions()).unwrap();
        let names: Vec<&str> = defs
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        for expected in [
            "get_schema",
            "sample_rows",
            "query",
            "create_table",
            "alter_table",
            "commit_records",
            "update_records",
            "delete_records",
        ] {
            assert!(names.contains(&expected), "missing tool: {expected}");
        }

        assert!(mcp.instructions().contains("crrdb"));

        let out = mcp.execute_tool("get_schema".into(), "{}".into()).unwrap();
        assert!(!out.is_error, "{}", out.json);
        assert!(out.json.contains("create_table"), "{}", out.json);

        let out = mcp
            .execute_tool(
                "create_table".into(),
                serde_json::json!({
                    "sql": "CREATE TABLE fuel (id TEXT PRIMARY KEY, liters REAL NOT NULL, _raw TEXT NOT NULL, _said_at TEXT NOT NULL) STRICT",
                    "description": "Fuel fill-ups",
                    "column_descriptions": {
                        "id": "ULID",
                        "liters": "Fuel volume in liters",
                        "_raw": "An utterance that created this row",
                        "_said_at": "Time of the utterance (ISO8601)"
                    },
                    "reason": "test"
                })
                .to_string(),
            )
            .unwrap();
        assert!(!out.is_error, "{}", out.json);

        let out = mcp
            .execute_tool(
                "commit_records".into(),
                serde_json::json!({"records": [{"table": "fuel", "values": {
                    "id": "01JGXK4YV0Q8Z3M9T2R5W7B1CD", "liters": 43.2,
                    "_raw": "43.2L", "_said_at": "2026-09-06T12:00:00Z"
                }}]})
                .to_string(),
            )
            .unwrap();
        assert!(!out.is_error, "{}", out.json);

        let out = mcp
            .execute_tool(
                "query".into(),
                serde_json::json!({"sql": "SELECT count(*) AS n FROM fuel"}).to_string(),
            )
            .unwrap();
        assert!(!out.is_error && out.json.contains("\"n\""), "{}", out.json);

        // Writes through `query` must be rejected.
        let out = mcp
            .execute_tool(
                "query".into(),
                serde_json::json!({"sql": "DELETE FROM fuel"}).to_string(),
            )
            .unwrap();
        assert!(out.is_error, "{}", out.json);

        std::fs::remove_dir_all(&dir).ok();
    }
}
