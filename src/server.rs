use std::sync::{Arc, Mutex};

use rmcp::{
    ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    serde_json::json,
    tool, tool_handler, tool_router,
};

use crate::{
    db::{Db, QueryOutput},
    error::Error,
    params::SampleRowsParams,
};

const INSTRUCTIONS: &str = r#"crrdb-mcp: a local database that can be accessible by
AI agents as well as traditional DB tools.

Conventions:
- Call get_schema first when touching the DB for the first time in a session.
- If there is no table that the client wants to use, call create_table.
- When a value doesn't fit an existing table, consider alter_table (ADD COLUMN) before
- The client is responsible for generating every row's primary key (id), which is
a 26-char ULID (e.g., 01JGXK4YV0Q8Z3M9T2R5W7B1CD). AUTOINCREMENT is forbidden since
crrdb supports conflict-free replication.
- Put the user's utterance into `_raw` column verbatim — never a summary. `_said_at`
column is the time of utterance, ISO8601.
- When one utterance produces multiple rows, send them all in a single commit_records
call (atomic).
- Column names in human language (English snake_case); enum-like values as text, not
integer codes; dates as ISO8601 text.
creating a new table.
- All arithmetic (sums, averages, returns) is done by SQL via query. Never compute
numbers yourself.
- Check the size with count before any large dump.
- Errors come back as JSON. Read kind/hint, fix the request, and retry once"#;

struct Server {
    db: Arc<Mutex<Db>>,
}

#[tool_handler]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("crrdb-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

#[tool_router]
impl Server {
    #[tool(
        description = "Return the full DDL plus natural-language descriptions of every \
        table and column. Call this first when touching the DB for the first time in a session."
    )]
    fn get_schema(&self) -> CallToolResult {
        tool_result(self.execute(|db| db.get_schema()).map(|maybe_schema| {
            match maybe_schema {
                Some(schema) => json!({"schema": schema}).to_string(),
                None => {
                    json!({"schema": "", "hint": "No table exists yet. Start with `create_table`"})
                        .to_string()
                }
            }
        }))
    }

    #[tool(description = "Return N rows (default 5) from a table. Use this \
        to get a feel for actual values — units, formatting conventions")]
    fn sample_rows(&self, Parameters(params): Parameters<SampleRowsParams>) -> CallToolResult {
        tool_result(
            self.execute(|db| db.sample_rows(&params.table, params.limit()))
                .map(|output| query_json(output).to_string()),
        )
    }
}

impl Server {
    fn execute<Output>(
        &self,
        func: impl FnOnce(&Db) -> Result<Output, Error>,
    ) -> Result<Output, Error> {
        let lock = match self.db.lock() {
            Ok(lock) => lock,
            Err(_) => {
                return Err(Error::DbLock);
            }
        };
        func(&lock)
    }
}

fn tool_result(result: Result<String, Error>) -> CallToolResult {
    match result {
        Ok(json) => CallToolResult::success(vec![ContentBlock::text(json)]),
        Err(e) => CallToolResult::error(vec![ContentBlock::text(e.to_json())]),
    }
}

fn query_json(output: QueryOutput) -> serde_json::Value {
    let rows = output
        .rows
        .iter()
        .map(|row| {
            serde_json::Value::Object(
                output
                    .columns
                    .iter()
                    .cloned()
                    .zip(row.iter().cloned())
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    if output.truncated {
        json!({
            "rows": rows,
            "count": rows.len(),
            "truncated": true,
            "notice": format!("The result was truncated, and only {} rows returned. Use aggregations or filters", rows.len()),
        })
    } else {
        json!({
            "rows": rows,
            "count": rows.len(),
            "truncated": false,
        })
    }
}
